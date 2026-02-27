use btleplug::api::ValueNotification;
use futures::stream::StreamExt;
use log::{error, warn};
use tokio::sync::{mpsc, watch};

use crate::commands::TrainerCommand;
use crate::connection::FtmsConnection;
use crate::constants::INDOOR_BIKE_DATA_UUID;
use crate::error::Result;
use crate::traits::BlePeripheral;
use crate::{ConnectionState, TrainerData};

/// Whether the background loop should continue or break.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum LoopAction {
    Continue,
    Break,
}

/// Handle a notification from the BLE peripheral.
///
/// Returns `LoopAction::Break` when the notification stream has ended.
pub(crate) fn handle_notification(
    notification: Option<ValueNotification>,
    data_tx: &watch::Sender<TrainerData>,
) -> LoopAction {
    match notification {
        Some(notif) if notif.uuid == INDOOR_BIKE_DATA_UUID => {
            match ftms_parser::parse_indoor_bike_data(&notif.value) {
                Ok(bike_data) => {
                    let _ = data_tx.send(TrainerData {
                        connection_state: ConnectionState::Connected,
                        bike_data: Some(bike_data),
                    });
                }
                Err(e) => {
                    warn!("Failed to parse indoor bike data: {:?}", e);
                }
            }
            LoopAction::Continue
        }
        Some(_) => LoopAction::Continue,
        None => {
            let _ = data_tx.send(TrainerData {
                connection_state: ConnectionState::Disconnected,
                bike_data: None,
            });
            LoopAction::Break
        }
    }
}

/// Send a disconnected state on the data channel.
pub(crate) fn send_disconnected(data_tx: &watch::Sender<TrainerData>) {
    let _ = data_tx.send(TrainerData {
        connection_state: ConnectionState::Disconnected,
        bike_data: None,
    });
}

/// Handle for communicating with a connected FTMS trainer.
///
/// Holds a command sender and a data receiver. The background task
/// owns the BLE connection and processes commands and notifications.
pub struct TrainerHandle {
    command_tx: mpsc::Sender<TrainerCommand>,
    data_rx: watch::Receiver<TrainerData>,
}

impl TrainerHandle {
    /// Set the target power in ERG mode (watts).
    pub async fn set_target_power(&self, watts: i16) -> std::result::Result<(), mpsc::error::SendError<TrainerCommand>> {
        self.command_tx
            .send(TrainerCommand::SetTargetPower(watts))
            .await
    }

    /// Set the target resistance level (raw uint8, 0.1 resolution).
    pub async fn set_target_resistance(&self, level: u8) -> std::result::Result<(), mpsc::error::SendError<TrainerCommand>> {
        self.command_tx
            .send(TrainerCommand::SetTargetResistance(level))
            .await
    }

    /// Set the target inclination (raw sint16, 0.1% resolution).
    pub async fn set_target_inclination(&self, inclination: i16) -> std::result::Result<(), mpsc::error::SendError<TrainerCommand>> {
        self.command_tx
            .send(TrainerCommand::SetTargetInclination(inclination))
            .await
    }

    /// Reset the trainer.
    pub async fn reset(&self) -> std::result::Result<(), mpsc::error::SendError<TrainerCommand>> {
        self.command_tx.send(TrainerCommand::Reset).await
    }

    /// Request disconnection from the trainer.
    pub async fn disconnect(&self) -> std::result::Result<(), mpsc::error::SendError<TrainerCommand>> {
        self.command_tx.send(TrainerCommand::Disconnect).await
    }

    /// Get a clone of the data receiver for observing trainer state.
    pub fn data_receiver(&self) -> watch::Receiver<TrainerData> {
        self.data_rx.clone()
    }
}

/// Connect to an FTMS trainer and spawn a background task to manage
/// the connection.
///
/// Returns a `TrainerHandle` for sending commands and observing data,
/// along with a `JoinHandle` for the background task.
pub async fn connect_to_trainer<P: BlePeripheral>(
    peripheral: P,
    data_tx: watch::Sender<TrainerData>,
    data_rx: watch::Receiver<TrainerData>,
) -> Result<(TrainerHandle, tokio::task::JoinHandle<()>)> {
    let mut connection = FtmsConnection::new(peripheral);
    let mut notification_stream = connection.connect_and_setup(&data_tx).await?;

    let (command_tx, mut command_rx) = mpsc::channel::<TrainerCommand>(32);

    let handle = TrainerHandle {
        command_tx,
        data_rx,
    };

    let join_handle = tokio::spawn(async move {
        loop {
            tokio::select! {
                notification = notification_stream.next() => {
                    if handle_notification(notification, &data_tx) == LoopAction::Break {
                        break;
                    }
                }
                command = command_rx.recv() => {
                    match command {
                        Some(TrainerCommand::Disconnect) => {
                            if let Err(e) = connection.disconnect().await {
                                error!("Disconnect error: {e}");
                            }
                            send_disconnected(&data_tx);
                            break;
                        }
                        Some(cmd) => {
                            if let Some(bytes) = cmd.serialize()
                                && let Err(e) = connection.write_control_command(
                                    &bytes,
                                    &mut notification_stream,
                                ).await
                            {
                                error!("Control command error: {e}");
                            }
                        }
                        None => {
                            break;
                        }
                    }
                }
            }
        }
    });

    Ok((handle, join_handle))
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

    fn feature_data() -> Vec<u8> {
        vec![0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]
    }

    fn cp_success_response() -> Vec<u8> {
        vec![0x80, 0x00, 0x01]
    }

    /// Build an Indoor Bike Data notification with speed, cadence, and power.
    fn bike_data_notification(speed_raw: u16, cadence_raw: u16, power: i16) -> ValueNotification {
        // Flags: INSTANTANEOUS_CADENCE | INSTANTANEOUS_POWER = 0x0044
        let flags: u16 = 0x0044;
        let mut data = Vec::new();
        data.extend_from_slice(&flags.to_le_bytes());
        data.extend_from_slice(&speed_raw.to_le_bytes());
        data.extend_from_slice(&cadence_raw.to_le_bytes());
        data.extend_from_slice(&power.to_le_bytes());
        ValueNotification {
            uuid: INDOOR_BIKE_DATA_UUID,
            value: data,
        }
    }

    /// A transport-test peripheral that provides configurable notifications
    /// via a tokio channel, so we can feed them during the test.
    struct TransportTestPeripheral {
        characteristics: BTreeSet<Characteristic>,
        notification_rx: Arc<Mutex<Option<tokio::sync::mpsc::Receiver<ValueNotification>>>>,
        write_log: Arc<Mutex<Vec<(uuid::Uuid, Vec<u8>)>>>,
        disconnected: Arc<Mutex<bool>>,
    }

    impl TransportTestPeripheral {
        fn new(
            notification_rx: tokio::sync::mpsc::Receiver<ValueNotification>,
        ) -> (Self, Arc<Mutex<Vec<(uuid::Uuid, Vec<u8>)>>>, Arc<Mutex<bool>>) {
            let write_log = Arc::new(Mutex::new(vec![]));
            let disconnected = Arc::new(Mutex::new(false));
            let peripheral = Self {
                characteristics: make_ftms_characteristics(),
                notification_rx: Arc::new(Mutex::new(Some(notification_rx))),
                write_log: write_log.clone(),
                disconnected: disconnected.clone(),
            };
            (peripheral, write_log, disconnected)
        }
    }

    #[async_trait]
    impl BlePeripheral for TransportTestPeripheral {
        async fn connect(&self) -> std::result::Result<(), btleplug::Error> {
            Ok(())
        }
        async fn disconnect(&self) -> std::result::Result<(), btleplug::Error> {
            *self.disconnected.lock().unwrap() = true;
            Ok(())
        }
        async fn is_connected(&self) -> std::result::Result<bool, btleplug::Error> {
            Ok(true)
        }
        async fn discover_services(&self) -> std::result::Result<(), btleplug::Error> {
            Ok(())
        }
        fn characteristics(&self) -> BTreeSet<Characteristic> {
            self.characteristics.clone()
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
            Ok(feature_data())
        }
        async fn write(
            &self,
            characteristic: &Characteristic,
            data: &[u8],
            _write_type: WriteType,
        ) -> std::result::Result<(), btleplug::Error> {
            self.write_log
                .lock()
                .unwrap()
                .push((characteristic.uuid, data.to_vec()));
            Ok(())
        }
        async fn subscribe(
            &self,
            _characteristic: &Characteristic,
        ) -> std::result::Result<(), btleplug::Error> {
            Ok(())
        }
        async fn notifications(
            &self,
        ) -> std::result::Result<
            Pin<Box<dyn futures::Stream<Item = ValueNotification> + Send>>,
            btleplug::Error,
        > {
            let rx = self.notification_rx.lock().unwrap().take().unwrap();
            let stream = tokio_stream::wrappers::ReceiverStream::new(rx);
            Ok(Box::pin(stream))
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn transport_receives_bike_data() {
        let (notif_tx, notif_rx) = tokio::sync::mpsc::channel(32);
        let (peripheral, _write_log, _disconnected) =
            TransportTestPeripheral::new(notif_rx);

        // Send the CP success response for connect_and_setup.
        notif_tx
            .send(ValueNotification {
                uuid: CONTROL_POINT_UUID,
                value: cp_success_response(),
            })
            .await
            .unwrap();

        let (data_tx, data_rx) = crate::trainer_data_channel();
        let (handle, join_handle) =
            connect_to_trainer(peripheral, data_tx, data_rx).await.unwrap();

        // Send a bike data notification.
        // Speed: 2500 = 25.00 km/h, Cadence: 180 = 90.0 rpm, Power: 200W
        notif_tx
            .send(bike_data_notification(2500, 180, 200))
            .await
            .unwrap();

        // Wait for bike data to propagate.
        let mut rx = handle.data_receiver();
        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                rx.changed().await.unwrap();
                if rx.borrow().bike_data.is_some() {
                    break;
                }
            }
        })
        .await
        .expect("should receive bike data");

        let data = rx.borrow().clone();
        assert_eq!(data.connection_state, ConnectionState::Connected);
        assert_eq!(data.bike_data.unwrap().instantaneous_power_watts, Some(200));

        // Disconnect.
        drop(notif_tx);
        join_handle.await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn transport_disconnect_command() {
        let (notif_tx, notif_rx) = tokio::sync::mpsc::channel(32);
        let (peripheral, _write_log, disconnected) =
            TransportTestPeripheral::new(notif_rx);

        notif_tx
            .send(ValueNotification {
                uuid: CONTROL_POINT_UUID,
                value: cp_success_response(),
            })
            .await
            .unwrap();

        let (data_tx, data_rx) = crate::trainer_data_channel();
        let (handle, join_handle) =
            connect_to_trainer(peripheral, data_tx, data_rx).await.unwrap();

        handle.disconnect().await.unwrap();
        join_handle.await.unwrap();

        assert!(*disconnected.lock().unwrap());
        let data = handle.data_receiver().borrow().clone();
        assert_eq!(data.connection_state, ConnectionState::Disconnected);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn transport_stream_end_sets_disconnected() {
        let (notif_tx, notif_rx) = tokio::sync::mpsc::channel(32);
        let (peripheral, _write_log, _disconnected) =
            TransportTestPeripheral::new(notif_rx);

        notif_tx
            .send(ValueNotification {
                uuid: CONTROL_POINT_UUID,
                value: cp_success_response(),
            })
            .await
            .unwrap();

        let (data_tx, data_rx) = crate::trainer_data_channel();
        let (handle, join_handle) =
            connect_to_trainer(peripheral, data_tx, data_rx).await.unwrap();

        // Drop the notification sender to end the stream.
        drop(notif_tx);
        join_handle.await.unwrap();

        let data = handle.data_receiver().borrow().clone();
        assert_eq!(data.connection_state, ConnectionState::Disconnected);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn transport_parse_error_does_not_crash() {
        let (notif_tx, notif_rx) = tokio::sync::mpsc::channel(32);
        let (peripheral, _write_log, _disconnected) =
            TransportTestPeripheral::new(notif_rx);

        notif_tx
            .send(ValueNotification {
                uuid: CONTROL_POINT_UUID,
                value: cp_success_response(),
            })
            .await
            .unwrap();

        let (data_tx, data_rx) = crate::trainer_data_channel();
        let (handle, join_handle) =
            connect_to_trainer(peripheral, data_tx, data_rx).await.unwrap();

        // Send an invalid bike data notification (too short).
        notif_tx
            .send(ValueNotification {
                uuid: INDOOR_BIKE_DATA_UUID,
                value: vec![0x00], // TooShort
            })
            .await
            .unwrap();

        // Send a valid one after.
        notif_tx
            .send(bike_data_notification(1000, 120, 100))
            .await
            .unwrap();

        // Wait for the valid data to arrive.
        let mut rx = handle.data_receiver();
        // The task should still be running and process the second notification.
        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                rx.changed().await.unwrap();
                if rx.borrow().bike_data.is_some() {
                    break;
                }
            }
        })
        .await
        .expect("should receive valid bike data after parse error");

        let data = rx.borrow().clone();
        assert_eq!(data.bike_data.as_ref().unwrap().instantaneous_power_watts, Some(100));

        drop(notif_tx);
        join_handle.await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn transport_sends_set_power_command() {
        let (notif_tx, notif_rx) = tokio::sync::mpsc::channel(32);
        let (peripheral, write_log, _disconnected) =
            TransportTestPeripheral::new(notif_rx);

        // CP success for connect_and_setup.
        notif_tx
            .send(ValueNotification {
                uuid: CONTROL_POINT_UUID,
                value: cp_success_response(),
            })
            .await
            .unwrap();

        let (data_tx, data_rx) = crate::trainer_data_channel();
        let (handle, join_handle) =
            connect_to_trainer(peripheral, data_tx, data_rx).await.unwrap();

        // Send a CP success for the set_target_power command.
        notif_tx
            .send(ValueNotification {
                uuid: CONTROL_POINT_UUID,
                value: vec![0x80, 0x05, 0x01], // success for SetTargetPower
            })
            .await
            .unwrap();

        handle.set_target_power(200).await.unwrap();

        // Give the background task time to process.
        tokio::time::sleep(Duration::from_millis(50)).await;

        let writes = write_log.lock().unwrap();
        // Should have Request Control + Set Target Power.
        assert!(writes.len() >= 2);
        let last = writes.last().unwrap();
        assert_eq!(last.0, CONTROL_POINT_UUID);
        assert_eq!(last.1, vec![0x05, 0xC8, 0x00]);

        drop(notif_tx);
        join_handle.await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn transport_ignores_non_bike_data_notifications() {
        let (notif_tx, notif_rx) = tokio::sync::mpsc::channel(32);
        let (peripheral, _write_log, _disconnected) =
            TransportTestPeripheral::new(notif_rx);

        notif_tx
            .send(ValueNotification {
                uuid: CONTROL_POINT_UUID,
                value: cp_success_response(),
            })
            .await
            .unwrap();

        let (data_tx, data_rx) = crate::trainer_data_channel();
        let (handle, join_handle) =
            connect_to_trainer(peripheral, data_tx, data_rx).await.unwrap();

        // Send a status notification (not bike data).
        notif_tx
            .send(ValueNotification {
                uuid: FITNESS_MACHINE_STATUS_UUID,
                value: vec![0x01],
            })
            .await
            .unwrap();

        // Send a real bike data notification.
        notif_tx
            .send(bike_data_notification(3000, 160, 150))
            .await
            .unwrap();

        let mut rx = handle.data_receiver();
        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                rx.changed().await.unwrap();
                if rx.borrow().bike_data.is_some() {
                    break;
                }
            }
        })
        .await
        .expect("should receive bike data");

        let data = rx.borrow().clone();
        assert_eq!(data.bike_data.as_ref().unwrap().instantaneous_power_watts, Some(150));

        drop(notif_tx);
        join_handle.await.unwrap();
    }

    #[tokio::test]
    async fn trainer_handle_set_target_resistance() {
        let (tx, mut rx) = mpsc::channel(1);
        let (_data_tx, data_rx) = crate::trainer_data_channel();
        let handle = TrainerHandle {
            command_tx: tx,
            data_rx,
        };
        handle.set_target_resistance(50).await.unwrap();
        let cmd = rx.recv().await.unwrap();
        assert_eq!(cmd, TrainerCommand::SetTargetResistance(50));
    }

    #[tokio::test]
    async fn trainer_handle_set_target_inclination() {
        let (tx, mut rx) = mpsc::channel(1);
        let (_data_tx, data_rx) = crate::trainer_data_channel();
        let handle = TrainerHandle {
            command_tx: tx,
            data_rx,
        };
        handle.set_target_inclination(-20).await.unwrap();
        let cmd = rx.recv().await.unwrap();
        assert_eq!(cmd, TrainerCommand::SetTargetInclination(-20));
    }

    #[tokio::test]
    async fn trainer_handle_reset() {
        let (tx, mut rx) = mpsc::channel(1);
        let (_data_tx, data_rx) = crate::trainer_data_channel();
        let handle = TrainerHandle {
            command_tx: tx,
            data_rx,
        };
        handle.reset().await.unwrap();
        let cmd = rx.recv().await.unwrap();
        assert_eq!(cmd, TrainerCommand::Reset);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn transport_command_channel_closed() {
        let (notif_tx, notif_rx) = tokio::sync::mpsc::channel(32);
        let (peripheral, _write_log, _disconnected) =
            TransportTestPeripheral::new(notif_rx);

        notif_tx
            .send(ValueNotification {
                uuid: CONTROL_POINT_UUID,
                value: cp_success_response(),
            })
            .await
            .unwrap();

        let (data_tx, data_rx) = crate::trainer_data_channel();
        let (handle, join_handle) =
            connect_to_trainer(peripheral, data_tx, data_rx).await.unwrap();

        // Drop the handle (and thus the command sender).
        drop(handle);

        // The background task should exit cleanly.
        tokio::time::timeout(Duration::from_secs(2), join_handle)
            .await
            .expect("task should complete")
            .unwrap();
    }

    // --- Unit tests for extracted handler functions ---

    #[test]
    fn handle_notification_bike_data() {
        let (data_tx, data_rx) = crate::trainer_data_channel();
        let notif = Some(bike_data_notification(2500, 180, 200));
        let action = handle_notification(notif, &data_tx);
        assert_eq!(action, LoopAction::Continue);
        let data = data_rx.borrow();
        assert_eq!(data.connection_state, ConnectionState::Connected);
        assert_eq!(data.bike_data.as_ref().unwrap().instantaneous_power_watts, Some(200));
    }

    #[test]
    fn handle_notification_parse_error() {
        let (data_tx, data_rx) = crate::trainer_data_channel();
        let notif = Some(ValueNotification {
            uuid: INDOOR_BIKE_DATA_UUID,
            value: vec![0x00], // Too short to parse
        });
        let action = handle_notification(notif, &data_tx);
        assert_eq!(action, LoopAction::Continue);
        // State should not change on parse error.
        assert_eq!(data_rx.borrow().connection_state, ConnectionState::Disconnected);
    }

    #[test]
    fn handle_notification_non_bike_data() {
        let (data_tx, data_rx) = crate::trainer_data_channel();
        let notif = Some(ValueNotification {
            uuid: FITNESS_MACHINE_STATUS_UUID,
            value: vec![0x01],
        });
        let action = handle_notification(notif, &data_tx);
        assert_eq!(action, LoopAction::Continue);
        assert_eq!(data_rx.borrow().connection_state, ConnectionState::Disconnected);
    }

    #[test]
    fn handle_notification_stream_ended() {
        let (data_tx, data_rx) = crate::trainer_data_channel();
        let action = handle_notification(None, &data_tx);
        assert_eq!(action, LoopAction::Break);
        assert_eq!(data_rx.borrow().connection_state, ConnectionState::Disconnected);
    }

    #[test]
    fn send_disconnected_sets_state() {
        let (data_tx, data_rx) = crate::trainer_data_channel();
        // First set to connected.
        let _ = data_tx.send(TrainerData {
            connection_state: ConnectionState::Connected,
            bike_data: None,
        });
        assert_eq!(data_rx.borrow().connection_state, ConnectionState::Connected);

        send_disconnected(&data_tx);
        assert_eq!(data_rx.borrow().connection_state, ConnectionState::Disconnected);
        assert!(data_rx.borrow().bike_data.is_none());
    }

    use std::time::Duration;
}
