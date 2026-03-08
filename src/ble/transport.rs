// libsmarttrainer - Library for controlling a bicycle smart trainer
// Copyright (C) 2026 Kris Hardy <hardyrk@gmail.com>
//
// This library is free software; you can redistribute it and/or
// modify it under the terms of the GNU Lesser General Public
// License as published by the Free Software Foundation; either
// version 2.1 of the License, or (at your option) any later version.
//
// This library is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
// Lesser General Public License for more details.
//
// You should have received a copy of the GNU Lesser General Public
// License along with this library; if not, write to the Free Software
// Foundation, Inc., 51 Franklin Street, Fifth Floor, Boston, MA  02110-1301
// USA

use btleplug::api::ValueNotification;
use crate::parser::IndoorBikeData;
use futures::stream::StreamExt;
use log::{error, info, warn};
use crate::safety::ErgSafetyMonitor;
use std::time::Instant;
use tokio::sync::{mpsc, watch};

use crate::ble::commands::TrainerCommand;
use crate::ble::connection::FtmsConnection;
use crate::ble::constants::INDOOR_BIKE_DATA_UUID;
use crate::ble::debounce::CommandDebouncer;
use crate::ble::error::Result;
use crate::ble::traits::BlePeripheral;
use crate::ble::{ConnectionState, TrainerData};

/// Check whether the trainer supports a given command based on parsed features.
///
/// Fail-open: if features were not parsed (e.g., feature read failed), the
/// command is allowed unconditionally. Only returns `Err` when the feature is
/// explicitly absent.
pub(crate) fn validate_command_feature(
    cmd: &TrainerCommand,
    features: &Option<crate::parser::FitnessMachineFeature>,
) -> std::result::Result<(), crate::ble::error::BleTransportError> {
    let Some(f) = features else {
        return Ok(());
    };
    match cmd {
        TrainerCommand::SetTargetPower(_) => {
            if !f.target_setting.contains(crate::parser::TargetSettingFeatures::POWER_TARGET) {
                return Err(crate::ble::error::BleTransportError::FeatureNotSupported(
                    "power target".into(),
                ));
            }
        }
        TrainerCommand::SetTargetResistance(_) => {
            if !f
                .target_setting
                .contains(crate::parser::TargetSettingFeatures::RESISTANCE_TARGET)
            {
                return Err(crate::ble::error::BleTransportError::FeatureNotSupported(
                    "resistance target".into(),
                ));
            }
        }
        TrainerCommand::SetIndoorBikeSimulation { .. } => {
            if !f
                .target_setting
                .contains(crate::parser::TargetSettingFeatures::INDOOR_BIKE_SIMULATION)
            {
                return Err(crate::ble::error::BleTransportError::FeatureNotSupported(
                    "indoor bike simulation".into(),
                ));
            }
        }
        _ => {}
    }
    Ok(())
}

/// Whether the background loop should continue or break.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum LoopAction {
    Continue,
    Break,
}

/// Handle a notification from the BLE peripheral.
///
/// Returns `(LoopAction, Option<IndoorBikeData>)`. The `LoopAction::Break`
/// signals that the notification stream has ended. The `IndoorBikeData` is
/// returned when a valid bike data notification is parsed, so the caller can
/// feed cadence to the safety monitor.
pub(crate) fn handle_notification(
    notification: Option<ValueNotification>,
    data_tx: &watch::Sender<TrainerData>,
) -> (LoopAction, Option<IndoorBikeData>) {
    match notification {
        Some(notif) if notif.uuid == INDOOR_BIKE_DATA_UUID => {
            match crate::parser::parse_indoor_bike_data(&notif.value) {
                Ok(mut bike_data) => {
                    crate::quirks::apply_default_quirks(&mut bike_data);
                    let _ = data_tx.send(TrainerData {
                        connection_state: ConnectionState::Connected,
                        bike_data: Some(bike_data.clone()),
                    });
                    (LoopAction::Continue, Some(bike_data))
                }
                Err(e) => {
                    warn!("Failed to parse indoor bike data: {:?}", e);
                    (LoopAction::Continue, None)
                }
            }
        }
        Some(_) => (LoopAction::Continue, None),
        None => {
            let _ = data_tx.send(TrainerData {
                connection_state: ConnectionState::Disconnected,
                bike_data: None,
            });
            (LoopAction::Break, None)
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

    /// Set indoor bike simulation parameters.
    pub async fn set_indoor_bike_simulation(
        &self,
        grade_001_pct: i16,
        crr: u8,
        cw: u8,
    ) -> std::result::Result<(), mpsc::error::SendError<TrainerCommand>> {
        self.command_tx
            .send(TrainerCommand::SetIndoorBikeSimulation { grade_001_pct, crr, cw })
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
    let parsed_features = connection.parsed_features().cloned();

    let (command_tx, mut command_rx) = mpsc::channel::<TrainerCommand>(32);

    let handle = TrainerHandle {
        command_tx,
        data_rx,
    };

    let join_handle = tokio::spawn(async move {
        let mut erg_monitor = ErgSafetyMonitor::new();
        let mut debouncer = CommandDebouncer::new();
        let mut ramp_interval = tokio::time::interval(std::time::Duration::from_millis(500));
        ramp_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            tokio::select! {
                notification = notification_stream.next() => {
                    let (action, bike_data) = handle_notification(notification, &data_tx);
                    if action == LoopAction::Break {
                        break;
                    }
                    // Feed cadence to the safety monitor.
                    if let Some(ref bd) = bike_data {
                        let now = Instant::now();
                        if let Some(power) = erg_monitor.on_cadence_update(bd.instantaneous_cadence_rpm, now) {
                            info!("ERG safety override: sending power {power}W");
                            let cmd = TrainerCommand::SetTargetPower(power);
                            if let Some(bytes) = cmd.serialize()
                                && let Err(e) = connection.write_control_command(
                                    &bytes,
                                    &mut notification_stream,
                                ).await
                            {
                                error!("ERG safety write error: {e}");
                            }
                            // Safety writes bypass the debouncer, but record
                            // the write so user commands respect the interval.
                            debouncer.record_write(Instant::now());
                        }
                    }
                }
                command = command_rx.recv() => {
                    match command {
                        Some(TrainerCommand::Disconnect) => {
                            erg_monitor.on_non_erg_command();
                            if let Err(e) = connection.disconnect().await {
                                error!("Disconnect error: {e}");
                            }
                            send_disconnected(&data_tx);
                            break;
                        }
                        Some(TrainerCommand::Reset) => {
                            // Reset bypasses the debouncer.
                            let cmd = TrainerCommand::Reset;
                            if let Some(bytes) = cmd.serialize()
                                && let Err(e) = connection.write_control_command(
                                    &bytes,
                                    &mut notification_stream,
                                ).await
                            {
                                error!("Control command error: {e}");
                            }
                        }
                        Some(TrainerCommand::SetTargetPower(watts)) => {
                            let cmd = TrainerCommand::SetTargetPower(watts);
                            if let Err(e) = validate_command_feature(&cmd, &parsed_features) {
                                warn!("{e}");
                            } else {
                                let now = Instant::now();
                                let actual = erg_monitor.on_set_target_power(watts, now);
                                let send = TrainerCommand::SetTargetPower(actual);
                                if let Some(send_cmd) = debouncer.submit(send, now)
                                    && let Some(bytes) = send_cmd.serialize()
                                    && let Err(e) = connection.write_control_command(
                                        &bytes,
                                        &mut notification_stream,
                                    ).await
                                {
                                    error!("Control command error: {e}");
                                }
                            }
                        }
                        Some(cmd) => {
                            if let Err(e) = validate_command_feature(&cmd, &parsed_features) {
                                warn!("{e}");
                            } else {
                                // Non-ERG control command — deactivate ERG safety,
                                // run through debouncer.
                                if matches!(cmd, TrainerCommand::SetTargetResistance(_)
                                    | TrainerCommand::SetIndoorBikeSimulation { .. })
                                {
                                    erg_monitor.on_non_erg_command();
                                }
                                let now = Instant::now();
                                if let Some(send_cmd) = debouncer.submit(cmd, now)
                                    && let Some(bytes) = send_cmd.serialize()
                                    && let Err(e) = connection.write_control_command(
                                        &bytes,
                                        &mut notification_stream,
                                    ).await
                                {
                                    error!("Control command error: {e}");
                                }
                            }
                        }
                        None => {
                            break;
                        }
                    }
                }
                // Debounce timer: fire pending command after interval elapses.
                _ = async {
                    match debouncer.time_until_next(Instant::now()) {
                        Some(d) => tokio::time::sleep(d).await,
                        None => std::future::pending::<()>().await,
                    }
                } => {
                    if let Some(cmd) = debouncer.poll_pending(Instant::now())
                        && let Some(bytes) = cmd.serialize()
                        && let Err(e) = connection.write_control_command(
                            &bytes,
                            &mut notification_stream,
                        ).await
                    {
                        error!("Debounced command write error: {e}");
                    }
                }
                _ = ramp_interval.tick(), if erg_monitor.needs_tick() => {
                    let now = Instant::now();
                    if let Some(power) = erg_monitor.on_ramp_tick(now) {
                        info!("ERG ramp tick: sending power {power}W");
                        let cmd = TrainerCommand::SetTargetPower(power);
                        if let Some(bytes) = cmd.serialize()
                            && let Err(e) = connection.write_control_command(
                                &bytes,
                                &mut notification_stream,
                            ).await
                        {
                            error!("ERG ramp write error: {e}");
                        }
                        // Ramp ticks are safety-critical; record the write.
                        debouncer.record_write(Instant::now());
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
    use crate::ble::constants::*;
    use crate::ble::traits::BlePeripheral;
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

    /// Feature data with POWER_TARGET | RESISTANCE_TARGET | INDOOR_BIKE_SIMULATION.
    /// Target setting bits: (1<<3) | (1<<2) | (1<<13) = 0x200C.
    fn feature_data() -> Vec<u8> {
        vec![0x00, 0x00, 0x00, 0x00, 0x0C, 0x20, 0x00, 0x00]
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

        let (data_tx, data_rx) = crate::ble::trainer_data_channel();
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

        let (data_tx, data_rx) = crate::ble::trainer_data_channel();
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

        let (data_tx, data_rx) = crate::ble::trainer_data_channel();
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

        let (data_tx, data_rx) = crate::ble::trainer_data_channel();
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

        let (data_tx, data_rx) = crate::ble::trainer_data_channel();
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

        let (data_tx, data_rx) = crate::ble::trainer_data_channel();
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
        let (_data_tx, data_rx) = crate::ble::trainer_data_channel();
        let handle = TrainerHandle {
            command_tx: tx,
            data_rx,
        };
        handle.set_target_resistance(50).await.unwrap();
        let cmd = rx.recv().await.unwrap();
        assert_eq!(cmd, TrainerCommand::SetTargetResistance(50));
    }

    #[tokio::test]
    async fn trainer_handle_set_indoor_bike_simulation() {
        let (tx, mut rx) = mpsc::channel(1);
        let (_data_tx, data_rx) = crate::ble::trainer_data_channel();
        let handle = TrainerHandle {
            command_tx: tx,
            data_rx,
        };
        handle.set_indoor_bike_simulation(-300, 40, 51).await.unwrap();
        let cmd = rx.recv().await.unwrap();
        assert_eq!(cmd, TrainerCommand::SetIndoorBikeSimulation {
            grade_001_pct: -300,
            crr: 40,
            cw: 51,
        });
    }

    #[tokio::test]
    async fn trainer_handle_reset() {
        let (tx, mut rx) = mpsc::channel(1);
        let (_data_tx, data_rx) = crate::ble::trainer_data_channel();
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

        let (data_tx, data_rx) = crate::ble::trainer_data_channel();
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
        let (data_tx, data_rx) = crate::ble::trainer_data_channel();
        let notif = Some(bike_data_notification(2500, 180, 200));
        let (action, bike_data) = handle_notification(notif, &data_tx);
        assert_eq!(action, LoopAction::Continue);
        assert!(bike_data.is_some());
        assert_eq!(bike_data.unwrap().instantaneous_power_watts, Some(200));
        let data = data_rx.borrow();
        assert_eq!(data.connection_state, ConnectionState::Connected);
        assert_eq!(data.bike_data.as_ref().unwrap().instantaneous_power_watts, Some(200));
    }

    #[test]
    fn handle_notification_parse_error() {
        let (data_tx, data_rx) = crate::ble::trainer_data_channel();
        let notif = Some(ValueNotification {
            uuid: INDOOR_BIKE_DATA_UUID,
            value: vec![0x00], // Too short to parse
        });
        let (action, bike_data) = handle_notification(notif, &data_tx);
        assert_eq!(action, LoopAction::Continue);
        assert!(bike_data.is_none());
        // State should not change on parse error.
        assert_eq!(data_rx.borrow().connection_state, ConnectionState::Disconnected);
    }

    #[test]
    fn handle_notification_non_bike_data() {
        let (data_tx, data_rx) = crate::ble::trainer_data_channel();
        let notif = Some(ValueNotification {
            uuid: FITNESS_MACHINE_STATUS_UUID,
            value: vec![0x01],
        });
        let (action, bike_data) = handle_notification(notif, &data_tx);
        assert_eq!(action, LoopAction::Continue);
        assert!(bike_data.is_none());
        assert_eq!(data_rx.borrow().connection_state, ConnectionState::Disconnected);
    }

    #[test]
    fn handle_notification_stream_ended() {
        let (data_tx, data_rx) = crate::ble::trainer_data_channel();
        let (action, bike_data) = handle_notification(None, &data_tx);
        assert_eq!(action, LoopAction::Break);
        assert!(bike_data.is_none());
        assert_eq!(data_rx.borrow().connection_state, ConnectionState::Disconnected);
    }

    #[test]
    fn send_disconnected_sets_state() {
        let (data_tx, data_rx) = crate::ble::trainer_data_channel();
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

    // --- Feature validation tests ---

    #[test]
    fn validate_allows_when_features_none() {
        let cmd = TrainerCommand::SetTargetPower(200);
        assert!(validate_command_feature(&cmd, &None).is_ok());
    }

    #[test]
    fn validate_allows_supported_power_target() {
        let features = Some(crate::parser::FitnessMachineFeature {
            fitness_machine: crate::parser::FitnessMachineFeatures::empty(),
            target_setting: crate::parser::TargetSettingFeatures::POWER_TARGET,
        });
        let cmd = TrainerCommand::SetTargetPower(200);
        assert!(validate_command_feature(&cmd, &features).is_ok());
    }

    #[test]
    fn validate_rejects_unsupported_power_target() {
        let features = Some(crate::parser::FitnessMachineFeature {
            fitness_machine: crate::parser::FitnessMachineFeatures::empty(),
            target_setting: crate::parser::TargetSettingFeatures::empty(),
        });
        let cmd = TrainerCommand::SetTargetPower(200);
        let err = validate_command_feature(&cmd, &features).unwrap_err();
        assert!(matches!(err, crate::ble::error::BleTransportError::FeatureNotSupported(_)));
    }

    #[test]
    fn validate_rejects_unsupported_resistance() {
        let features = Some(crate::parser::FitnessMachineFeature {
            fitness_machine: crate::parser::FitnessMachineFeatures::empty(),
            target_setting: crate::parser::TargetSettingFeatures::POWER_TARGET,
        });
        let cmd = TrainerCommand::SetTargetResistance(50);
        let err = validate_command_feature(&cmd, &features).unwrap_err();
        assert!(matches!(err, crate::ble::error::BleTransportError::FeatureNotSupported(_)));
    }

    #[test]
    fn validate_rejects_unsupported_simulation() {
        let features = Some(crate::parser::FitnessMachineFeature {
            fitness_machine: crate::parser::FitnessMachineFeatures::empty(),
            target_setting: crate::parser::TargetSettingFeatures::POWER_TARGET,
        });
        let cmd = TrainerCommand::SetIndoorBikeSimulation {
            grade_001_pct: 500,
            crr: 40,
            cw: 51,
        };
        let err = validate_command_feature(&cmd, &features).unwrap_err();
        assert!(matches!(err, crate::ble::error::BleTransportError::FeatureNotSupported(_)));
    }

    #[test]
    fn validate_allows_disconnect_and_reset() {
        let features = Some(crate::parser::FitnessMachineFeature {
            fitness_machine: crate::parser::FitnessMachineFeatures::empty(),
            target_setting: crate::parser::TargetSettingFeatures::empty(),
        });
        assert!(validate_command_feature(&TrainerCommand::Disconnect, &features).is_ok());
        assert!(validate_command_feature(&TrainerCommand::Reset, &features).is_ok());
    }
}
