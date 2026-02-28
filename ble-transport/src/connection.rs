use std::pin::Pin;
use std::time::Duration;

use btleplug::api::{Characteristic, ValueNotification, WriteType};
use futures::stream::{Stream, StreamExt};
use log::info;
use tokio::sync::watch;

use crate::constants::{
    CONTROL_POINT_UUID, FEATURE_UUID, FITNESS_MACHINE_STATUS_UUID, INDOOR_BIKE_DATA_UUID,
};
use crate::error::{BleTransportError, Result};
use crate::traits::BlePeripheral;
use crate::{ConnectionState, TrainerData};

/// Cached characteristic handles discovered during FTMS setup.
pub struct FtmsCharacteristics {
    pub indoor_bike_data: Characteristic,
    pub control_point: Characteristic,
    pub feature: Characteristic,
    pub status: Option<Characteristic>,
}

/// An FTMS connection to a BLE peripheral.
///
/// Manages the full FTMS handshake: connect, discover services, find
/// characteristics, read features, subscribe to notifications, and
/// request control.
pub struct FtmsConnection<P: BlePeripheral> {
    peripheral: P,
    characteristics: Option<FtmsCharacteristics>,
}

impl<P: BlePeripheral> FtmsConnection<P> {
    pub fn new(peripheral: P) -> Self {
        Self {
            peripheral,
            characteristics: None,
        }
    }

    /// Perform the full FTMS connection and setup sequence.
    ///
    /// 1. Connect to the peripheral
    /// 2. Discover services
    /// 3. Find required characteristics by UUID
    /// 4. Read the Feature characteristic (0x2ACC)
    /// 5. Subscribe to Indoor Bike Data (0x2AD2), Control Point (0x2AD9),
    ///    and Status (0x2ADA)
    /// 6. Write Request Control to the Control Point
    /// 7. Wait for success indication
    pub async fn connect_and_setup(
        &mut self,
        data_tx: &watch::Sender<TrainerData>,
    ) -> Result<Pin<Box<dyn Stream<Item = ValueNotification> + Send>>> {
        // Signal connecting state.
        let _ = data_tx.send(TrainerData {
            connection_state: ConnectionState::Connecting,
            bike_data: None,
        });

        // Step 1: Connect.
        self.peripheral.connect().await?;

        // Step 2: Discover services.
        self.peripheral.discover_services().await?;

        // Step 3: Find characteristics.
        let chars = self.peripheral.characteristics();
        let chars_vec: Vec<Characteristic> = chars.into_iter().collect();

        let indoor_bike_data = find_characteristic(&chars_vec, INDOOR_BIKE_DATA_UUID)?;
        let control_point = find_characteristic(&chars_vec, CONTROL_POINT_UUID)?;
        let feature = find_characteristic(&chars_vec, FEATURE_UUID)?;
        let status = find_characteristic(&chars_vec, FITNESS_MACHINE_STATUS_UUID).ok();

        // Step 4: Read Feature characteristic.
        let feature_data = self.peripheral.read(&feature).await?;
        if let Ok(features) = ftms_parser::parse_feature(&feature_data) {
            info!("Machine features: {:?}", features.fitness_machine);
            info!("Target settings:  {:?}", features.target_setting);
        }

        // Step 5: Subscribe to notifications.
        self.peripheral.subscribe(&indoor_bike_data).await?;
        self.peripheral.subscribe(&control_point).await?;
        if let Some(ref status_char) = status {
            self.peripheral.subscribe(status_char).await?;
        }

        // Get notification stream before writing control point.
        let mut stream = self.peripheral.notifications().await?;

        // Step 6: Write Request Control.
        let request_control = ftms_parser::serialize_control_point_request_control();
        self.peripheral
            .write(&control_point, &request_control, WriteType::WithResponse)
            .await?;

        // Step 7: Wait for success indication.
        let resp = wait_cp_response(&mut stream, CONTROL_POINT_UUID).await?;
        if resp.result_code != ftms_parser::ControlPointResultCode::Success {
            return Err(BleTransportError::ControlPointRejected(resp));
        }

        // Cache characteristics.
        self.characteristics = Some(FtmsCharacteristics {
            indoor_bike_data,
            control_point,
            feature,
            status,
        });

        // Signal connected state.
        let _ = data_tx.send(TrainerData {
            connection_state: ConnectionState::Connected,
            bike_data: None,
        });

        Ok(stream)
    }

    /// Write a control command to the FTMS Control Point and wait for indication.
    pub async fn write_control_command(
        &self,
        data: &[u8],
        stream: &mut Pin<Box<dyn Stream<Item = ValueNotification> + Send>>,
    ) -> Result<()> {
        let chars = self
            .characteristics
            .as_ref()
            .ok_or(BleTransportError::NotConnected)?;

        self.peripheral
            .write(&chars.control_point, data, WriteType::WithResponse)
            .await?;

        let resp = wait_cp_response(stream, CONTROL_POINT_UUID).await?;
        if resp.result_code != ftms_parser::ControlPointResultCode::Success {
            return Err(BleTransportError::ControlPointRejected(resp));
        }

        Ok(())
    }

    /// Disconnect from the peripheral.
    pub async fn disconnect(&self) -> Result<()> {
        self.peripheral.disconnect().await?;
        Ok(())
    }
}

/// Find a characteristic by UUID from a list of discovered characteristics.
pub fn find_characteristic(
    characteristics: &[Characteristic],
    uuid: uuid::Uuid,
) -> Result<Characteristic> {
    characteristics
        .iter()
        .find(|c| c.uuid == uuid)
        .cloned()
        .ok_or_else(|| BleTransportError::CharacteristicNotFound(uuid.to_string()))
}

/// Wait for a Control Point indication response on the notification stream.
///
/// Times out after 5 seconds.
async fn wait_cp_response(
    stream: &mut Pin<Box<dyn Stream<Item = ValueNotification> + Send>>,
    control_point_uuid: uuid::Uuid,
) -> Result<ftms_parser::ControlPointResponse> {
    let timeout_duration = Duration::from_secs(10);
    let result = tokio::time::timeout(timeout_duration, async {
        while let Some(notification) = stream.next().await {
            if notification.uuid == control_point_uuid {
                return ftms_parser::parse_control_point_response(&notification.value)
                    .map_err(BleTransportError::from);
            }
        }
        Err(BleTransportError::StreamEnded)
    })
    .await;

    match result {
        Ok(inner) => inner,
        Err(_) => Err(BleTransportError::Timeout),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::*;
    use crate::traits::BlePeripheral;
    use async_trait::async_trait;
    use btleplug::api::{
        CharPropFlags, Characteristic, PeripheralProperties, Service, ValueNotification, WriteType,
    };
    use std::collections::BTreeSet;
    use std::pin::Pin;
    use std::sync::{Arc, Mutex};

    fn make_characteristic(uuid: uuid::Uuid, service_uuid: uuid::Uuid) -> Characteristic {
        Characteristic {
            uuid,
            service_uuid,
            properties: CharPropFlags::default(),
            descriptors: BTreeSet::new(),
        }
    }

    fn make_ftms_characteristics() -> BTreeSet<Characteristic> {
        let mut chars = BTreeSet::new();
        chars.insert(make_characteristic(INDOOR_BIKE_DATA_UUID, FTMS_SERVICE_UUID));
        chars.insert(make_characteristic(CONTROL_POINT_UUID, FTMS_SERVICE_UUID));
        chars.insert(make_characteristic(FEATURE_UUID, FTMS_SERVICE_UUID));
        chars.insert(make_characteristic(
            FITNESS_MACHINE_STATUS_UUID,
            FTMS_SERVICE_UUID,
        ));
        chars
    }

    /// Feature data: all zeroes (valid 8-byte feature characteristic)
    fn feature_data() -> Vec<u8> {
        vec![0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]
    }

    /// Control point success response for Request Control (op code 0x00)
    fn cp_success_response() -> Vec<u8> {
        vec![0x80, 0x00, 0x01]
    }

    /// Control point not-supported response
    fn cp_not_supported_response() -> Vec<u8> {
        vec![0x80, 0x00, 0x02]
    }

    // --- Configurable Mock Peripheral ---

    struct MockPeripheralConfig {
        characteristics: BTreeSet<Characteristic>,
        feature_data: Vec<u8>,
        connect_fails: bool,
        notifications: Vec<ValueNotification>,
        write_log: Arc<Mutex<Vec<(uuid::Uuid, Vec<u8>)>>>,
        subscribe_log: Arc<Mutex<Vec<uuid::Uuid>>>,
    }

    impl Default for MockPeripheralConfig {
        fn default() -> Self {
            Self {
                characteristics: make_ftms_characteristics(),
                feature_data: feature_data(),
                connect_fails: false,
                notifications: vec![ValueNotification {
                    uuid: CONTROL_POINT_UUID,
                    value: cp_success_response(),
                }],
                write_log: Arc::new(Mutex::new(vec![])),
                subscribe_log: Arc::new(Mutex::new(vec![])),
            }
        }
    }

    struct TestPeripheral {
        config: MockPeripheralConfig,
    }

    impl TestPeripheral {
        fn new(config: MockPeripheralConfig) -> Self {
            Self { config }
        }
    }

    #[async_trait]
    impl BlePeripheral for TestPeripheral {
        async fn connect(&self) -> std::result::Result<(), btleplug::Error> {
            if self.config.connect_fails {
                Err(btleplug::Error::DeviceNotFound)
            } else {
                Ok(())
            }
        }
        async fn disconnect(&self) -> std::result::Result<(), btleplug::Error> {
            Ok(())
        }
        async fn is_connected(&self) -> std::result::Result<bool, btleplug::Error> {
            Ok(true)
        }
        async fn discover_services(&self) -> std::result::Result<(), btleplug::Error> {
            Ok(())
        }
        fn characteristics(&self) -> BTreeSet<Characteristic> {
            self.config.characteristics.clone()
        }
        fn services(&self) -> BTreeSet<Service> {
            BTreeSet::new()
        }
        async fn properties(
            &self,
        ) -> std::result::Result<Option<PeripheralProperties>, btleplug::Error> {
            Ok(None)
        }
        async fn read(
            &self,
            _characteristic: &Characteristic,
        ) -> std::result::Result<Vec<u8>, btleplug::Error> {
            Ok(self.config.feature_data.clone())
        }
        async fn write(
            &self,
            characteristic: &Characteristic,
            data: &[u8],
            _write_type: WriteType,
        ) -> std::result::Result<(), btleplug::Error> {
            self.config
                .write_log
                .lock()
                .unwrap()
                .push((characteristic.uuid, data.to_vec()));
            Ok(())
        }
        async fn subscribe(
            &self,
            characteristic: &Characteristic,
        ) -> std::result::Result<(), btleplug::Error> {
            self.config
                .subscribe_log
                .lock()
                .unwrap()
                .push(characteristic.uuid);
            Ok(())
        }
        async fn notifications(
            &self,
        ) -> std::result::Result<
            Pin<Box<dyn futures::Stream<Item = ValueNotification> + Send>>,
            btleplug::Error,
        > {
            let notifs = self.config.notifications.clone();
            Ok(Box::pin(futures::stream::iter(notifs)))
        }
    }

    // --- Tests ---

    #[tokio::test]
    async fn connect_and_setup_success() {
        let config = MockPeripheralConfig::default();
        let write_log = config.write_log.clone();
        let subscribe_log = config.subscribe_log.clone();
        let peripheral = TestPeripheral::new(config);
        let mut conn = FtmsConnection::new(peripheral);
        let (tx, rx) = crate::trainer_data_channel();

        let _stream = conn.connect_and_setup(&tx).await.unwrap();

        // Verify connected state.
        assert_eq!(rx.borrow().connection_state, ConnectionState::Connected);

        // Verify subscriptions were made for all 4 characteristics.
        let subs = subscribe_log.lock().unwrap();
        assert!(subs.contains(&INDOOR_BIKE_DATA_UUID));
        assert!(subs.contains(&CONTROL_POINT_UUID));
        assert!(subs.contains(&FITNESS_MACHINE_STATUS_UUID));

        // Verify Request Control was written.
        let writes = write_log.lock().unwrap();
        assert_eq!(writes.len(), 1);
        assert_eq!(writes[0].0, CONTROL_POINT_UUID);
        assert_eq!(writes[0].1, vec![0x00]); // Request Control op code
    }

    #[tokio::test]
    async fn connect_and_setup_transitions_through_connecting() {
        let config = MockPeripheralConfig::default();
        let peripheral = TestPeripheral::new(config);
        let mut conn = FtmsConnection::new(peripheral);
        let (tx, _rx) = crate::trainer_data_channel();

        // Before connect: Disconnected.
        assert_eq!(
            _rx.borrow().connection_state,
            ConnectionState::Disconnected
        );

        let _stream = conn.connect_and_setup(&tx).await.unwrap();

        // After connect: Connected.
        assert_eq!(_rx.borrow().connection_state, ConnectionState::Connected);
    }

    #[tokio::test]
    async fn connect_and_setup_missing_bike_data_characteristic() {
        let mut config = MockPeripheralConfig::default();
        config.characteristics = {
            let mut chars = BTreeSet::new();
            chars.insert(make_characteristic(CONTROL_POINT_UUID, FTMS_SERVICE_UUID));
            chars.insert(make_characteristic(FEATURE_UUID, FTMS_SERVICE_UUID));
            chars
        };
        let peripheral = TestPeripheral::new(config);
        let mut conn = FtmsConnection::new(peripheral);
        let (tx, _rx) = crate::trainer_data_channel();

        let result = conn.connect_and_setup(&tx).await;
        assert!(matches!(
            result,
            Err(BleTransportError::CharacteristicNotFound(_))
        ));
    }

    #[tokio::test]
    async fn connect_and_setup_missing_control_point() {
        let mut config = MockPeripheralConfig::default();
        config.characteristics = {
            let mut chars = BTreeSet::new();
            chars.insert(make_characteristic(
                INDOOR_BIKE_DATA_UUID,
                FTMS_SERVICE_UUID,
            ));
            chars.insert(make_characteristic(FEATURE_UUID, FTMS_SERVICE_UUID));
            chars
        };
        let peripheral = TestPeripheral::new(config);
        let mut conn = FtmsConnection::new(peripheral);
        let (tx, _rx) = crate::trainer_data_channel();

        let result = conn.connect_and_setup(&tx).await;
        assert!(matches!(
            result,
            Err(BleTransportError::CharacteristicNotFound(_))
        ));
    }

    #[tokio::test]
    async fn connect_and_setup_missing_feature() {
        let mut config = MockPeripheralConfig::default();
        config.characteristics = {
            let mut chars = BTreeSet::new();
            chars.insert(make_characteristic(
                INDOOR_BIKE_DATA_UUID,
                FTMS_SERVICE_UUID,
            ));
            chars.insert(make_characteristic(CONTROL_POINT_UUID, FTMS_SERVICE_UUID));
            chars
        };
        let peripheral = TestPeripheral::new(config);
        let mut conn = FtmsConnection::new(peripheral);
        let (tx, _rx) = crate::trainer_data_channel();

        let result = conn.connect_and_setup(&tx).await;
        assert!(matches!(
            result,
            Err(BleTransportError::CharacteristicNotFound(_))
        ));
    }

    #[tokio::test]
    async fn connect_and_setup_control_point_rejected() {
        let mut config = MockPeripheralConfig::default();
        config.notifications = vec![ValueNotification {
            uuid: CONTROL_POINT_UUID,
            value: cp_not_supported_response(),
        }];
        let peripheral = TestPeripheral::new(config);
        let mut conn = FtmsConnection::new(peripheral);
        let (tx, _rx) = crate::trainer_data_channel();

        let result = conn.connect_and_setup(&tx).await;
        assert!(matches!(
            result,
            Err(BleTransportError::ControlPointRejected(_))
        ));
    }

    #[tokio::test]
    async fn connect_and_setup_connect_fails() {
        let mut config = MockPeripheralConfig::default();
        config.connect_fails = true;
        let peripheral = TestPeripheral::new(config);
        let mut conn = FtmsConnection::new(peripheral);
        let (tx, _rx) = crate::trainer_data_channel();

        let result = conn.connect_and_setup(&tx).await;
        assert!(matches!(result, Err(BleTransportError::Btleplug(_))));
    }

    #[tokio::test]
    async fn connect_and_setup_without_status_characteristic() {
        let mut config = MockPeripheralConfig::default();
        config.characteristics = {
            let mut chars = BTreeSet::new();
            chars.insert(make_characteristic(
                INDOOR_BIKE_DATA_UUID,
                FTMS_SERVICE_UUID,
            ));
            chars.insert(make_characteristic(CONTROL_POINT_UUID, FTMS_SERVICE_UUID));
            chars.insert(make_characteristic(FEATURE_UUID, FTMS_SERVICE_UUID));
            // No status characteristic — should still succeed.
            chars
        };
        let subscribe_log = config.subscribe_log.clone();
        let peripheral = TestPeripheral::new(config);
        let mut conn = FtmsConnection::new(peripheral);
        let (tx, _rx) = crate::trainer_data_channel();

        let _stream = conn.connect_and_setup(&tx).await.unwrap();

        // Status should not have been subscribed.
        let subs = subscribe_log.lock().unwrap();
        assert!(!subs.contains(&FITNESS_MACHINE_STATUS_UUID));
    }

    #[tokio::test]
    async fn write_control_command_success() {
        let config = MockPeripheralConfig::default();
        let write_log = config.write_log.clone();
        let peripheral = TestPeripheral::new(config);
        let mut conn = FtmsConnection::new(peripheral);
        let (tx, _rx) = crate::trainer_data_channel();

        let _stream = conn.connect_and_setup(&tx).await.unwrap();

        // Set up a new stream with a fresh success response for the next write.
        let mut new_stream: Pin<Box<dyn Stream<Item = ValueNotification> + Send>> =
            Box::pin(futures::stream::iter(vec![ValueNotification {
                uuid: CONTROL_POINT_UUID,
                value: cp_success_response(),
            }]));

        let set_power = ftms_parser::serialize_control_point_set_target_power(200);
        conn.write_control_command(&set_power, &mut new_stream)
            .await
            .unwrap();

        let writes = write_log.lock().unwrap();
        // First write is Request Control, second is Set Target Power.
        assert_eq!(writes.len(), 2);
        assert_eq!(writes[1].0, CONTROL_POINT_UUID);
        assert_eq!(writes[1].1, vec![0x05, 0xC8, 0x00]);
    }

    #[tokio::test]
    async fn write_control_command_not_connected() {
        let config = MockPeripheralConfig::default();
        let peripheral = TestPeripheral::new(config);
        let conn = FtmsConnection::new(peripheral);
        // Don't call connect_and_setup — characteristics are None.

        let mut stream: Pin<Box<dyn Stream<Item = ValueNotification> + Send>> =
            Box::pin(futures::stream::empty());

        let result = conn
            .write_control_command(&[0x00], &mut stream)
            .await;
        assert!(matches!(result, Err(BleTransportError::NotConnected)));
    }

    #[tokio::test]
    async fn write_control_command_rejected() {
        let config = MockPeripheralConfig::default();
        let peripheral = TestPeripheral::new(config);
        let mut conn = FtmsConnection::new(peripheral);
        let (tx, _rx) = crate::trainer_data_channel();

        let _stream = conn.connect_and_setup(&tx).await.unwrap();

        let mut reject_stream: Pin<Box<dyn Stream<Item = ValueNotification> + Send>> =
            Box::pin(futures::stream::iter(vec![ValueNotification {
                uuid: CONTROL_POINT_UUID,
                value: cp_not_supported_response(),
            }]));

        let result = conn
            .write_control_command(&[0x01], &mut reject_stream)
            .await;
        assert!(matches!(
            result,
            Err(BleTransportError::ControlPointRejected(_))
        ));
    }

    #[tokio::test]
    async fn write_control_command_stream_ended() {
        let config = MockPeripheralConfig::default();
        let peripheral = TestPeripheral::new(config);
        let mut conn = FtmsConnection::new(peripheral);
        let (tx, _rx) = crate::trainer_data_channel();

        let _stream = conn.connect_and_setup(&tx).await.unwrap();

        let mut empty_stream: Pin<Box<dyn Stream<Item = ValueNotification> + Send>> =
            Box::pin(futures::stream::empty());

        let result = conn
            .write_control_command(&[0x00], &mut empty_stream)
            .await;
        assert!(matches!(result, Err(BleTransportError::StreamEnded)));
    }

    #[test]
    fn find_characteristic_found() {
        let chars = vec![
            make_characteristic(INDOOR_BIKE_DATA_UUID, FTMS_SERVICE_UUID),
            make_characteristic(CONTROL_POINT_UUID, FTMS_SERVICE_UUID),
        ];
        let found = find_characteristic(&chars, CONTROL_POINT_UUID).unwrap();
        assert_eq!(found.uuid, CONTROL_POINT_UUID);
    }

    #[test]
    fn find_characteristic_not_found() {
        let chars = vec![make_characteristic(
            INDOOR_BIKE_DATA_UUID,
            FTMS_SERVICE_UUID,
        )];
        let result = find_characteristic(&chars, CONTROL_POINT_UUID);
        assert!(matches!(
            result,
            Err(BleTransportError::CharacteristicNotFound(_))
        ));
    }

    #[test]
    fn find_characteristic_empty_list() {
        let result = find_characteristic(&[], FEATURE_UUID);
        assert!(matches!(
            result,
            Err(BleTransportError::CharacteristicNotFound(_))
        ));
    }

    #[tokio::test]
    async fn wait_cp_response_ignores_non_cp_notifications() {
        let notifications = vec![
            ValueNotification {
                uuid: INDOOR_BIKE_DATA_UUID,
                value: vec![0x00, 0x00, 0x00, 0x00],
            },
            ValueNotification {
                uuid: CONTROL_POINT_UUID,
                value: vec![0x80, 0x00, 0x01], // success
            },
        ];
        let mut stream: Pin<Box<dyn Stream<Item = ValueNotification> + Send>> =
            Box::pin(futures::stream::iter(notifications));

        let resp = wait_cp_response(&mut stream, CONTROL_POINT_UUID)
            .await
            .unwrap();
        assert_eq!(
            resp.result_code,
            ftms_parser::ControlPointResultCode::Success
        );
    }

    #[tokio::test]
    async fn wait_cp_response_stream_ends() {
        let mut stream: Pin<Box<dyn Stream<Item = ValueNotification> + Send>> =
            Box::pin(futures::stream::empty());

        let result = wait_cp_response(&mut stream, CONTROL_POINT_UUID).await;
        assert!(matches!(result, Err(BleTransportError::StreamEnded)));
    }

    #[tokio::test]
    async fn wait_cp_response_parse_error() {
        let notifications = vec![ValueNotification {
            uuid: CONTROL_POINT_UUID,
            value: vec![0x00], // invalid: not 0x80 prefix and too short
        }];
        let mut stream: Pin<Box<dyn Stream<Item = ValueNotification> + Send>> =
            Box::pin(futures::stream::iter(notifications));

        let result = wait_cp_response(&mut stream, CONTROL_POINT_UUID).await;
        assert!(matches!(result, Err(BleTransportError::Parse(_))));
    }

    #[tokio::test]
    async fn wait_cp_response_timeout() {
        tokio::time::pause();
        // A pending stream never produces items, triggering the 5s timeout.
        let mut stream: Pin<Box<dyn Stream<Item = ValueNotification> + Send>> =
            Box::pin(futures::stream::pending());

        let result = wait_cp_response(&mut stream, CONTROL_POINT_UUID).await;
        assert!(matches!(result, Err(BleTransportError::Timeout)));
    }

    #[tokio::test]
    async fn disconnect_succeeds() {
        let config = MockPeripheralConfig::default();
        let peripheral = TestPeripheral::new(config);
        let conn = FtmsConnection::new(peripheral);
        conn.disconnect().await.unwrap();
    }
}
