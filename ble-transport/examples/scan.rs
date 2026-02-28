//! Discover FTMS-compatible smart trainers over BLE.
//!
//! Run with: `cargo run -p ble-transport --example scan`

use std::time::Duration;

use ble_transport::{get_adapter, scan_for_ftms_devices};

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

    if devices.is_empty() {
        println!("No FTMS trainers found. Make sure your trainer is powered on.");
        return;
    }

    println!("Found {} device(s):\n", devices.len());
    println!("{:<30} {}", "Name", "Address");
    println!("{:<30} {}", "----", "-------");
    for (device, _peripheral) in &devices {
        println!("{:<30} {}", device.name, device.address);
    }
}
