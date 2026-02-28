//! Stream real-time bike data from a connected FTMS trainer.
//!
//! Prints speed, cadence, power, and heart rate as they arrive.
//! Press Ctrl+C to disconnect and exit.
//!
//! Run with: `cargo run -p ble-transport --example read_data`

use std::time::Duration;

use ble_transport::{
    connect_to_trainer, get_adapter, scan_for_ftms_devices, trainer_data_channel, ConnectionState,
};

#[tokio::main]
async fn main() {
    let adapter = match get_adapter().await {
        Ok(a) => a,
        Err(e) => {
            eprintln!("Error: {}", e.user_message());
            std::process::exit(1);
        }
    };

    println!("Scanning for FTMS trainers (5 seconds)...");
    let devices = match scan_for_ftms_devices(&adapter, Duration::from_secs(5)).await {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Scan error: {}", e.user_message());
            std::process::exit(1);
        }
    };

    let (device, peripheral) = match devices.into_iter().next() {
        Some(pair) => pair,
        None => {
            eprintln!("No FTMS trainers found.");
            std::process::exit(1);
        }
    };

    println!("Connecting to {} ({})...", device.name, device.address);
    let (data_tx, data_rx) = trainer_data_channel();
    let (handle, join) = match connect_to_trainer(peripheral, data_tx, data_rx).await {
        Ok(pair) => pair,
        Err(e) => {
            eprintln!("Connection error: {}", e.user_message());
            std::process::exit(1);
        }
    };
    println!("Connected! Streaming data (Ctrl+C to stop)...\n");

    println!(
        "{:<12} {:<12} {:<12} {:<12} {:<12}",
        "State", "Speed", "Cadence", "Power", "HR"
    );
    println!("{}", "-".repeat(60));

    let mut rx = handle.data_receiver();

    loop {
        tokio::select! {
            result = rx.changed() => {
                if result.is_err() {
                    println!("Data channel closed.");
                    break;
                }
                let data = rx.borrow_and_update().clone();

                let state = match data.connection_state {
                    ConnectionState::Connected => "Connected",
                    ConnectionState::Connecting => "Connecting",
                    ConnectionState::Reconnecting => "Reconnecting",
                    ConnectionState::Disconnected => "Disconnected",
                };

                if let Some(bike) = &data.bike_data {
                    let speed = bike
                        .instantaneous_speed_kmh
                        .map(|v| format!("{v:.1} km/h"))
                        .unwrap_or_else(|| "--".into());
                    let cadence = bike
                        .instantaneous_cadence_rpm
                        .map(|v| format!("{v:.0} rpm"))
                        .unwrap_or_else(|| "--".into());
                    let power = bike
                        .instantaneous_power_watts
                        .map(|v| format!("{v} W"))
                        .unwrap_or_else(|| "--".into());
                    let hr = bike
                        .heart_rate_bpm
                        .map(|v| format!("{v} bpm"))
                        .unwrap_or_else(|| "--".into());

                    println!("{state:<12} {speed:<12} {cadence:<12} {power:<12} {hr:<12}");
                }
            }
            _ = tokio::signal::ctrl_c() => {
                println!("\nDisconnecting...");
                break;
            }
        }
    }

    let _ = handle.disconnect().await;
    let _ = join.await;
    println!("Disconnected.");
}
