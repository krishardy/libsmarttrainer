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

use crate::parser::{ControlPointResponse, ParseError};

/// Errors from BLE transport operations.
#[derive(Debug, thiserror::Error)]
pub enum BleTransportError {
    #[error("no Bluetooth adapter found")]
    NoAdapter,

    #[error("no FTMS device found")]
    NoDeviceFound,

    #[error("characteristic {0} not found")]
    CharacteristicNotFound(String),

    #[error("Bluetooth permission denied")]
    PermissionDenied,

    #[error("BLE error: {0}")]
    Btleplug(btleplug::Error),

    #[error("control point rejected: {0:?}")]
    ControlPointRejected(ControlPointResponse),

    #[error("parse error: {0}")]
    Parse(ParseError),

    #[error("control point response timed out")]
    Timeout,

    #[error("notification stream ended unexpectedly")]
    StreamEnded,

    #[error("not connected to device")]
    NotConnected,

    #[error("trainer does not support {0}")]
    FeatureNotSupported(String),
}

impl From<btleplug::Error> for BleTransportError {
    fn from(e: btleplug::Error) -> Self {
        match e {
            btleplug::Error::PermissionDenied => BleTransportError::PermissionDenied,
            other => BleTransportError::Btleplug(other),
        }
    }
}

impl From<ParseError> for BleTransportError {
    fn from(e: ParseError) -> Self {
        BleTransportError::Parse(e)
    }
}

impl BleTransportError {
    /// Return a user-friendly error message with recovery hints.
    pub fn user_message(&self) -> String {
        match self {
            BleTransportError::PermissionDenied => {
                "Bluetooth permission denied. On Linux, ensure your user is in the 'bluetooth' group.".into()
            }
            BleTransportError::NoAdapter => {
                "No Bluetooth adapter found. Check that Bluetooth is enabled.".into()
            }
            BleTransportError::NoDeviceFound => {
                "No FTMS trainer found. Make sure your trainer is powered on and in range.".into()
            }
            BleTransportError::CharacteristicNotFound(_) => {
                "Required characteristic not found. The device may not support FTMS.".into()
            }
            BleTransportError::Timeout => {
                "Connection timed out. Move closer to the trainer and try again.".into()
            }
            BleTransportError::NotConnected => {
                "Not connected to a trainer. Connect to a device first.".into()
            }
            BleTransportError::StreamEnded => {
                "Lost connection to trainer. The device may have powered off.".into()
            }
            BleTransportError::ControlPointRejected(_) => {
                "Trainer rejected command. The feature may not be supported.".into()
            }
            BleTransportError::FeatureNotSupported(feature) => {
                format!("Your trainer does not support {feature}. Try a different control mode.")
            }
            BleTransportError::Parse(e) => {
                format!("Data parsing error: {e}.")
            }
            BleTransportError::Btleplug(e) => {
                let msg = e.to_string();
                if msg.contains("Service discovery timed out") {
                    "Service discovery timed out. Try connecting again — this is a transient Bluetooth issue.".into()
                } else {
                    format!("Bluetooth error: {msg}")
                }
            }
        }
    }
}

/// Convenience type alias for BLE transport results.
pub type Result<T> = std::result::Result<T, BleTransportError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_no_adapter() {
        let err = BleTransportError::NoAdapter;
        assert_eq!(err.to_string(), "no Bluetooth adapter found");
    }

    #[test]
    fn display_characteristic_not_found() {
        let err = BleTransportError::CharacteristicNotFound("0x2AD2".to_string());
        assert_eq!(err.to_string(), "characteristic 0x2AD2 not found");
    }

    #[test]
    fn display_timeout() {
        let err = BleTransportError::Timeout;
        assert_eq!(err.to_string(), "control point response timed out");
    }

    #[test]
    fn display_stream_ended() {
        let err = BleTransportError::StreamEnded;
        assert_eq!(err.to_string(), "notification stream ended unexpectedly");
    }

    #[test]
    fn display_not_connected() {
        let err = BleTransportError::NotConnected;
        assert_eq!(err.to_string(), "not connected to device");
    }

    #[test]
    fn display_no_device_found() {
        let err = BleTransportError::NoDeviceFound;
        assert_eq!(err.to_string(), "no FTMS device found");
    }

    #[test]
    fn display_permission_denied() {
        let err = BleTransportError::PermissionDenied;
        assert_eq!(err.to_string(), "Bluetooth permission denied");
    }

    #[test]
    fn from_parse_error() {
        let parse_err = ParseError::TooShort;
        let err: BleTransportError = parse_err.into();
        assert!(matches!(err, BleTransportError::Parse(ParseError::TooShort)));
    }

    #[test]
    fn from_parse_error_invalid_data() {
        let parse_err = ParseError::InvalidData;
        let err: BleTransportError = parse_err.into();
        assert!(matches!(
            err,
            BleTransportError::Parse(ParseError::InvalidData)
        ));
    }

    #[test]
    fn from_btleplug_permission_denied() {
        let btl_err = btleplug::Error::PermissionDenied;
        let err: BleTransportError = btl_err.into();
        assert!(matches!(err, BleTransportError::PermissionDenied));
    }

    #[test]
    fn from_btleplug_other_error() {
        let btl_err = btleplug::Error::DeviceNotFound;
        let err: BleTransportError = btl_err.into();
        assert!(matches!(err, BleTransportError::Btleplug(_)));
    }

    #[test]
    fn display_control_point_rejected() {
        use crate::parser::{ControlPointResultCode, ControlPointResponse};
        let resp = ControlPointResponse {
            request_op_code: 0x05,
            result_code: ControlPointResultCode::NotSupported,
        };
        let err = BleTransportError::ControlPointRejected(resp);
        let msg = err.to_string();
        assert!(msg.contains("control point rejected"));
    }

    #[test]
    fn display_parse_error() {
        let err = BleTransportError::Parse(ParseError::TooShort);
        let msg = err.to_string();
        assert!(msg.contains("parse error"));
        // Now uses Display (not Debug), so should show the human message.
        assert!(msg.contains("data payload too short"));
    }

    // ── user_message() tests ──────────────────────────────────

    #[test]
    fn user_message_permission_denied() {
        let err = BleTransportError::PermissionDenied;
        let msg = err.user_message();
        assert!(msg.contains("Bluetooth permission denied"));
        assert!(msg.contains("bluetooth"));
    }

    #[test]
    fn user_message_no_adapter() {
        let err = BleTransportError::NoAdapter;
        let msg = err.user_message();
        assert!(msg.contains("No Bluetooth adapter found"));
        assert!(msg.contains("enabled"));
    }

    #[test]
    fn user_message_no_device_found() {
        let err = BleTransportError::NoDeviceFound;
        let msg = err.user_message();
        assert!(msg.contains("No FTMS trainer found"));
        assert!(msg.contains("powered on"));
    }

    #[test]
    fn user_message_characteristic_not_found() {
        let err = BleTransportError::CharacteristicNotFound("0x2AD2".into());
        let msg = err.user_message();
        assert!(msg.contains("Required characteristic not found"));
    }

    #[test]
    fn user_message_timeout() {
        let err = BleTransportError::Timeout;
        let msg = err.user_message();
        assert!(msg.contains("timed out"));
        assert!(msg.contains("closer"));
    }

    #[test]
    fn user_message_not_connected() {
        let err = BleTransportError::NotConnected;
        let msg = err.user_message();
        assert!(msg.contains("Not connected"));
    }

    #[test]
    fn user_message_stream_ended() {
        let err = BleTransportError::StreamEnded;
        let msg = err.user_message();
        assert!(msg.contains("Lost connection"));
    }

    #[test]
    fn user_message_control_point_rejected() {
        use crate::parser::{ControlPointResultCode, ControlPointResponse};
        let resp = ControlPointResponse {
            request_op_code: 0x05,
            result_code: ControlPointResultCode::NotSupported,
        };
        let err = BleTransportError::ControlPointRejected(resp);
        let msg = err.user_message();
        assert!(msg.contains("rejected"));
    }

    #[test]
    fn user_message_parse_error() {
        let err = BleTransportError::Parse(ParseError::TooShort);
        let msg = err.user_message();
        assert!(msg.contains("Data parsing error"));
        assert!(msg.contains("data payload too short"));
    }

    #[test]
    fn user_message_btleplug() {
        let err = BleTransportError::Btleplug(btleplug::Error::DeviceNotFound);
        let msg = err.user_message();
        assert!(msg.contains("Bluetooth error"));
    }

    #[test]
    fn user_message_service_discovery_timed_out() {
        let err = BleTransportError::Btleplug(btleplug::Error::Other(
            "Service discovery timed out".into(),
        ));
        let msg = err.user_message();
        assert!(msg.contains("Service discovery timed out"));
        assert!(msg.contains("transient Bluetooth issue"));
    }

    #[test]
    fn display_feature_not_supported() {
        let err = BleTransportError::FeatureNotSupported("power target".into());
        assert_eq!(err.to_string(), "trainer does not support power target");
    }

    #[test]
    fn user_message_feature_not_supported() {
        let err = BleTransportError::FeatureNotSupported("power target".into());
        let msg = err.user_message();
        assert!(msg.contains("does not support power target"));
        assert!(msg.contains("different control mode"));
    }
}
