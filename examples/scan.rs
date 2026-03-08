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

//! Discover FTMS-compatible smart trainers over BLE.
//!
//! Run with: `cargo run --example scan`

use std::time::Duration;

use libsmarttrainer::ble::{get_adapter, scan_for_ftms_devices};

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
