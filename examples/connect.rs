//! Connect to the first discovered FTMS trainer and hold the connection.
//!
//! Press Ctrl+C to disconnect and exit.
//!
//! Run with: `cargo run --example connect`

use std::time::Duration;

use libsmarttrainer::ble::{
    connect_to_trainer, get_adapter, scan_for_ftms_devices, trainer_data_channel,
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

    println!("Connected! Press Ctrl+C to disconnect.");

    tokio::signal::ctrl_c()
        .await
        .expect("Failed to listen for Ctrl+C");

    println!("\nDisconnecting...");
    let _ = handle.disconnect().await;
    let _ = join.await;
    println!("Disconnected.");
}
