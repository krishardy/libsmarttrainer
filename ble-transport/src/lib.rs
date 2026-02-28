pub mod commands;
pub mod connection;
pub mod constants;
pub mod debounce;
pub mod error;
pub mod scanner;
pub mod traits;
pub mod transport;

use ftms_parser::IndoorBikeData;

// Re-export key public types.
pub use commands::TrainerCommand;
pub use connection::FtmsConnection;
pub use constants::*;
pub use error::BleTransportError;
pub use scanner::{get_adapter, scan_for_ftms_devices, DiscoveredDevice};
pub use traits::{BleAdapter, BlePeripheral, BtleplugAdapter, BtleplugPeripheral};
pub use transport::{connect_to_trainer, TrainerHandle};

/// BLE connection state for the trainer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Reconnecting,
}

/// Latest data received from a connected trainer.
#[derive(Debug, Clone)]
pub struct TrainerData {
    pub connection_state: ConnectionState,
    pub bike_data: Option<IndoorBikeData>,
}

impl Default for TrainerData {
    fn default() -> Self {
        Self {
            connection_state: ConnectionState::Disconnected,
            bike_data: None,
        }
    }
}

/// Type alias for the trainer data receiver (watch channel).
pub type TrainerDataReceiver = tokio::sync::watch::Receiver<TrainerData>;

/// Create a `tokio::sync::watch` channel pair for streaming trainer data.
///
/// The sender should be held by the BLE connection task. The receiver can be
/// cloned and shared with UI or logging consumers. Only the latest value is
/// retained — consumers always see the most recent reading.
pub fn trainer_data_channel() -> (
    tokio::sync::watch::Sender<TrainerData>,
    tokio::sync::watch::Receiver<TrainerData>,
) {
    tokio::sync::watch::channel(TrainerData::default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_trainer_data() {
        let data = TrainerData::default();
        assert_eq!(data.connection_state, ConnectionState::Disconnected);
        assert!(data.bike_data.is_none());
    }

    #[test]
    fn connection_state_equality() {
        assert_eq!(ConnectionState::Connected, ConnectionState::Connected);
        assert_ne!(ConnectionState::Connecting, ConnectionState::Disconnected);
        assert_eq!(ConnectionState::Reconnecting, ConnectionState::Reconnecting);
        assert_ne!(ConnectionState::Reconnecting, ConnectionState::Connected);
    }

    #[test]
    fn channel_creation() {
        let (_tx, rx) = trainer_data_channel();
        let initial = rx.borrow();
        assert_eq!(initial.connection_state, ConnectionState::Disconnected);
        assert!(initial.bike_data.is_none());
    }

    #[tokio::test]
    async fn channel_send_receive() {
        let (tx, mut rx) = trainer_data_channel();
        let bike_data = IndoorBikeData {
            instantaneous_speed_kmh: Some(25.0),
            instantaneous_cadence_rpm: Some(90.0),
            instantaneous_power_watts: Some(200),
            heart_rate_bpm: None,
        };
        tx.send(TrainerData {
            connection_state: ConnectionState::Connected,
            bike_data: Some(bike_data.clone()),
        })
        .unwrap();

        rx.changed().await.unwrap();
        let received = rx.borrow();
        assert_eq!(received.connection_state, ConnectionState::Connected);
        let received_bike = received.bike_data.as_ref().unwrap();
        assert_eq!(received_bike.instantaneous_power_watts, Some(200));
    }
}
