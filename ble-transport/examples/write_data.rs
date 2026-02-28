//! Send control commands to a connected FTMS trainer.
//!
//! Demonstrates all three control modes:
//! - ERG (target power in watts)
//! - Resistance (target resistance level)
//! - Indoor Bike Simulation (grade, rolling resistance, wind resistance)
//!
//! Run with: `cargo run -p ble-transport --example write_data`

use std::time::Duration;

use ble_transport::{
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
    println!("Connected!\n");

    // --- ERG mode: set target power ---
    // The trainer adjusts brake resistance to maintain the target wattage
    // regardless of cadence.
    println!("Setting ERG target: 150 watts...");
    if let Err(e) = handle.set_target_power(150).await {
        eprintln!("Failed to set target power: {e}");
    }
    tokio::time::sleep(Duration::from_secs(3)).await;

    // --- Resistance mode: set target resistance level ---
    // Raw uint8 value with 0.1 resolution. 50 = resistance level 5.0.
    println!("Setting resistance level: 50 (= 5.0)...");
    if let Err(e) = handle.set_target_resistance(50).await {
        eprintln!("Failed to set resistance: {e}");
    }
    tokio::time::sleep(Duration::from_secs(3)).await;

    // --- Simulation mode: indoor bike simulation ---
    // Parameters:
    //   grade_001_pct: grade in 0.01% units (500 = 5.00% grade)
    //   crr: rolling resistance coefficient in 0.0001 units (40 = 0.0040)
    //   cw: wind resistance coefficient in 0.01 kg/m units (51 = 0.51 kg/m)
    println!("Setting simulation: 5.00% grade, crr=0.0040, cw=0.51...");
    if let Err(e) = handle.set_indoor_bike_simulation(500, 40, 51).await {
        eprintln!("Failed to set simulation: {e}");
    }
    tokio::time::sleep(Duration::from_secs(3)).await;

    // --- Reset and disconnect ---
    println!("Resetting trainer...");
    let _ = handle.reset().await;
    tokio::time::sleep(Duration::from_secs(1)).await;

    println!("Disconnecting...");
    let _ = handle.disconnect().await;
    let _ = join.await;
    println!("Done.");
}
