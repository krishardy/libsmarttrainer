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

//! `libsmarttrainer` — Rust library for controlling BLE smart trainers via the
//! FTMS (Fitness Machine Service) protocol.
//!
//! # Feature flags
//!
//! - **`ble`** *(enabled by default)* — includes the full BLE transport layer
//!   (`btleplug`, `tokio`, scanning, connection management, command debouncing).
//!   Disable with `default-features = false` if you only need the lightweight
//!   parser, quirks, and safety modules.

pub mod parser;
pub mod quirks;
pub mod safety;

#[cfg(feature = "ble")]
pub mod ble;

// Re-export commonly used types at crate root.
pub use parser::{
    parse_indoor_bike_data, parse_control_point_response, parse_feature,
    serialize_control_point_request_control, serialize_control_point_reset,
    serialize_control_point_set_indoor_bike_simulation, serialize_control_point_set_target_power,
    serialize_control_point_set_target_resistance,
    ControlPointResponse, ControlPointResultCode, FitnessMachineFeature, FitnessMachineFeatures,
    IndoorBikeData, IndoorBikeDataFlags, ParseError, TargetSettingFeatures,
};
pub use quirks::apply_default_quirks;
pub use safety::ErgSafetyMonitor;

#[cfg(feature = "ble")]
pub use ble::{
    connect_to_trainer, get_adapter, scan_for_ftms_devices, trainer_data_channel,
    BleTransportError, ConnectionState, DiscoveredDevice, TrainerCommand, TrainerData,
    TrainerDataReceiver, TrainerHandle,
};
