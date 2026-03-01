//! End-to-end quick start: scan, connect, write, read, disconnect.
//!
//! Runs a time-bounded workflow (no Ctrl+C needed):
//!   1. Scan for trainers
//!   2. Connect to the first one found
//!   3. Set ERG target to 150 watts
//!   4. Read data for 10 seconds
//!   5. Disconnect
//!
//! Run with: `cargo run --example full_workflow`

use std::time::Duration;

use libsmarttrainer::ble::{
    connect_to_trainer, get_adapter, scan_for_ftms_devices, trainer_data_channel, ConnectionState,
};

#[tokio::main]
async fn main() {
    // Step 1: Get the Bluetooth adapter and scan for FTMS trainers.
    println!("Step 1: Scanning for FTMS trainers (5 seconds)...");
    let adapter = match get_adapter().await {
        Ok(a) => a,
        Err(e) => {
            eprintln!("Error: {}", e.user_message());
            std::process::exit(1);
        }
    };

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
    println!("  Found: {} ({})\n", device.name, device.address);

    // Step 2: Connect to the trainer.
    println!("Step 2: Connecting...");
    let (data_tx, data_rx) = trainer_data_channel();
    let (handle, join) = match connect_to_trainer(peripheral, data_tx, data_rx).await {
        Ok(pair) => pair,
        Err(e) => {
            eprintln!("Connection error: {}", e.user_message());
            std::process::exit(1);
        }
    };
    println!("  Connected!\n");

    // Step 3: Set ERG target to 150 watts.
    println!("Step 3: Setting ERG target to 150W...");
    if let Err(e) = handle.set_target_power(150).await {
        eprintln!("  Failed: {e}");
    } else {
        println!("  Target set.\n");
    }

    // Step 4: Read data for 10 seconds.
    println!("Step 4: Reading data for 10 seconds...");
    println!(
        "  {:<12} {:<12} {:<12} {:<12}",
        "Speed", "Cadence", "Power", "HR"
    );

    let mut rx = handle.data_receiver();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);

    loop {
        tokio::select! {
            result = rx.changed() => {
                if result.is_err() {
                    println!("  Data channel closed.");
                    break;
                }
                let data = rx.borrow_and_update().clone();
                if data.connection_state != ConnectionState::Connected {
                    continue;
                }
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

                    println!("  {speed:<12} {cadence:<12} {power:<12} {hr:<12}");
                }
            }
            _ = tokio::time::sleep_until(deadline) => {
                println!("\n  10 seconds elapsed.");
                break;
            }
        }
    }

    // Step 5: Disconnect.
    println!("\nStep 5: Disconnecting...");
    let _ = handle.disconnect().await;
    let _ = join.await;
    println!("  Done.");
}
