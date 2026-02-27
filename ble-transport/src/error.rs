use ftms_parser::{ControlPointResponse, ParseError};

/// Errors from BLE transport operations.
#[derive(Debug, thiserror::Error)]
pub enum BleTransportError {
    #[error("no Bluetooth adapter found")]
    NoAdapter,

    #[error("no FTMS device found")]
    NoDeviceFound,

    #[error("characteristic {0} not found")]
    CharacteristicNotFound(String),

    #[error("BLE error: {0}")]
    Btleplug(#[from] btleplug::Error),

    #[error("control point rejected: {0:?}")]
    ControlPointRejected(ControlPointResponse),

    #[error("parse error: {0:?}")]
    Parse(ParseError),

    #[error("control point response timed out")]
    Timeout,

    #[error("notification stream ended unexpectedly")]
    StreamEnded,

    #[error("not connected to device")]
    NotConnected,
}

impl From<ParseError> for BleTransportError {
    fn from(e: ParseError) -> Self {
        BleTransportError::Parse(e)
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
    fn display_control_point_rejected() {
        use ftms_parser::{ControlPointResultCode, ControlPointResponse};
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
    }
}
