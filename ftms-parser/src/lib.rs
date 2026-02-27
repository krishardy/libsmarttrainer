#![no_std]

use bitflags::bitflags;

bitflags! {
    /// Flags from the Indoor Bike Data characteristic (0x2AD2).
    ///
    /// Each flag indicates whether the corresponding field is present in the
    /// notification payload. Bit 0 controls speed: when **clear**, instantaneous
    /// speed is present (it is present by default).
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct IndoorBikeDataFlags: u16 {
        /// When clear, instantaneous speed is present (default present).
        const MORE_DATA              = 0b0000_0000_0000_0001;
        const AVERAGE_SPEED          = 0b0000_0000_0000_0010;
        const INSTANTANEOUS_CADENCE  = 0b0000_0000_0000_0100;
        const AVERAGE_CADENCE        = 0b0000_0000_0000_1000;
        const TOTAL_DISTANCE         = 0b0000_0000_0001_0000;
        const RESISTANCE_LEVEL       = 0b0000_0000_0010_0000;
        const INSTANTANEOUS_POWER    = 0b0000_0000_0100_0000;
        const AVERAGE_POWER          = 0b0000_0000_1000_0000;
        const EXPENDED_ENERGY        = 0b0000_0001_0000_0000;
        const HEART_RATE             = 0b0000_0010_0000_0000;
        const METABOLIC_EQUIVALENT   = 0b0000_0100_0000_0000;
        const ELAPSED_TIME           = 0b0000_1000_0000_0000;
        const REMAINING_TIME         = 0b0001_0000_0000_0000;
    }
}

/// Parsed fields from an Indoor Bike Data notification.
#[derive(Debug, Clone, PartialEq)]
pub struct IndoorBikeData {
    /// Instantaneous speed in km/h (resolution 0.01).
    pub instantaneous_speed_kmh: Option<f64>,
    /// Instantaneous cadence in rpm (resolution 0.5).
    pub instantaneous_cadence_rpm: Option<f64>,
    /// Instantaneous power in watts.
    pub instantaneous_power_watts: Option<i16>,
    /// Heart rate in bpm.
    pub heart_rate_bpm: Option<u8>,
}

/// Errors returned by parser functions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    /// The payload is too short to contain the required fields.
    TooShort,
    /// The payload contains an invalid or unsupported value.
    InvalidData,
}

/// Parse a raw Indoor Bike Data (0x2AD2) notification payload.
///
/// Returns the parsed data fields based on the flags present in the first two
/// bytes.
pub fn parse_indoor_bike_data(data: &[u8]) -> Result<IndoorBikeData, ParseError> {
    if data.len() < 2 {
        return Err(ParseError::TooShort);
    }

    let flags = IndoorBikeDataFlags::from_bits_truncate(u16::from_le_bytes([data[0], data[1]]));
    let mut offset: usize = 2;

    // Instantaneous speed: present when MORE_DATA flag is NOT set.
    let instantaneous_speed_kmh = if !flags.contains(IndoorBikeDataFlags::MORE_DATA) {
        if data.len() < offset + 2 {
            return Err(ParseError::TooShort);
        }
        let raw = u16::from_le_bytes([data[offset], data[offset + 1]]);
        offset += 2;
        Some(f64::from(raw) * 0.01)
    } else {
        None
    };

    // Average speed
    if flags.contains(IndoorBikeDataFlags::AVERAGE_SPEED) {
        if data.len() < offset + 2 {
            return Err(ParseError::TooShort);
        }
        offset += 2;
    }

    // Instantaneous cadence
    let instantaneous_cadence_rpm = if flags.contains(IndoorBikeDataFlags::INSTANTANEOUS_CADENCE) {
        if data.len() < offset + 2 {
            return Err(ParseError::TooShort);
        }
        let raw = u16::from_le_bytes([data[offset], data[offset + 1]]);
        offset += 2;
        Some(f64::from(raw) * 0.5)
    } else {
        None
    };

    // Average cadence
    if flags.contains(IndoorBikeDataFlags::AVERAGE_CADENCE) {
        if data.len() < offset + 2 {
            return Err(ParseError::TooShort);
        }
        offset += 2;
    }

    // Total distance (3 bytes)
    if flags.contains(IndoorBikeDataFlags::TOTAL_DISTANCE) {
        if data.len() < offset + 3 {
            return Err(ParseError::TooShort);
        }
        offset += 3;
    }

    // Resistance level
    if flags.contains(IndoorBikeDataFlags::RESISTANCE_LEVEL) {
        if data.len() < offset + 2 {
            return Err(ParseError::TooShort);
        }
        offset += 2;
    }

    // Instantaneous power
    let instantaneous_power_watts = if flags.contains(IndoorBikeDataFlags::INSTANTANEOUS_POWER) {
        if data.len() < offset + 2 {
            return Err(ParseError::TooShort);
        }
        let raw = i16::from_le_bytes([data[offset], data[offset + 1]]);
        offset += 2;
        Some(raw)
    } else {
        None
    };

    // Average power
    if flags.contains(IndoorBikeDataFlags::AVERAGE_POWER) {
        if data.len() < offset + 2 {
            return Err(ParseError::TooShort);
        }
        offset += 2;
    }

    // Expended energy (total 2 bytes + per hour 2 bytes + per minute 1 byte)
    if flags.contains(IndoorBikeDataFlags::EXPENDED_ENERGY) {
        if data.len() < offset + 5 {
            return Err(ParseError::TooShort);
        }
        offset += 5;
    }

    // Heart rate
    let heart_rate_bpm = if flags.contains(IndoorBikeDataFlags::HEART_RATE) {
        if data.len() < offset + 1 {
            return Err(ParseError::TooShort);
        }
        let hr = data[offset];
        // offset += 1; // not needed, last field we read
        Some(hr)
    } else {
        None
    };

    Ok(IndoorBikeData {
        instantaneous_speed_kmh,
        instantaneous_cadence_rpm,
        instantaneous_power_watts,
        heart_rate_bpm,
    })
}

/// Serialize a Fitness Machine Control Point "Request Control" command (op code
/// 0x00).
///
/// Returns the raw bytes to write to the Control Point characteristic (0x2AD9).
pub fn serialize_control_point_request_control() -> [u8; 1] {
    [0x00]
}

/// Parse the Fitness Machine Feature characteristic (0x2ACC).
///
/// Returns the raw feature bitfield as a `u32`. Full flag interpretation is
/// deferred to a later task.
pub fn parse_feature(data: &[u8]) -> Result<u32, ParseError> {
    if data.len() < 4 {
        return Err(ParseError::TooShort);
    }
    Ok(u32::from_le_bytes([data[0], data[1], data[2], data[3]]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_indoor_bike_data_too_short() {
        assert_eq!(parse_indoor_bike_data(&[]), Err(ParseError::TooShort));
        assert_eq!(parse_indoor_bike_data(&[0x00]), Err(ParseError::TooShort));
    }

    #[test]
    fn parse_indoor_bike_data_speed_only() {
        // Flags: 0x0000 => MORE_DATA not set, so speed is present. No other
        // flags set.
        // Speed: 2500 => 25.00 km/h
        let data = [0x00, 0x00, 0xC4, 0x09];
        let result = parse_indoor_bike_data(&data).unwrap();
        assert!((result.instantaneous_speed_kmh.unwrap() - 25.0).abs() < 0.01);
        assert_eq!(result.instantaneous_cadence_rpm, None);
        assert_eq!(result.instantaneous_power_watts, None);
        assert_eq!(result.heart_rate_bpm, None);
    }

    #[test]
    fn parse_indoor_bike_data_speed_cadence_power() {
        // Flags: INSTANTANEOUS_CADENCE | INSTANTANEOUS_POWER = 0x0044
        // Speed: 3000 => 30.00 km/h
        // Cadence: 180 => 90.0 rpm
        // Power: 200W
        let data = [0x44, 0x00, 0xB8, 0x0B, 0xB4, 0x00, 0xC8, 0x00];
        let result = parse_indoor_bike_data(&data).unwrap();
        assert!((result.instantaneous_speed_kmh.unwrap() - 30.0).abs() < 0.01);
        assert!((result.instantaneous_cadence_rpm.unwrap() - 90.0).abs() < 0.1);
        assert_eq!(result.instantaneous_power_watts, Some(200));
    }

    #[test]
    fn parse_indoor_bike_data_truncated_speed() {
        // Flags say speed is present (MORE_DATA not set) but payload too short.
        let data = [0x00, 0x00, 0xC4];
        assert_eq!(parse_indoor_bike_data(&data), Err(ParseError::TooShort));
    }

    #[test]
    fn serialize_request_control() {
        assert_eq!(serialize_control_point_request_control(), [0x00]);
    }

    #[test]
    fn parse_feature_valid() {
        let data = [0x01, 0x02, 0x03, 0x04];
        assert_eq!(parse_feature(&data), Ok(0x04030201));
    }

    #[test]
    fn parse_feature_too_short() {
        assert_eq!(parse_feature(&[0x01, 0x02]), Err(ParseError::TooShort));
    }

    #[test]
    fn bitflags_round_trip() {
        let flags = IndoorBikeDataFlags::INSTANTANEOUS_CADENCE
            | IndoorBikeDataFlags::INSTANTANEOUS_POWER;
        assert_eq!(flags.bits(), 0x0044);
        assert!(flags.contains(IndoorBikeDataFlags::INSTANTANEOUS_CADENCE));
        assert!(!flags.contains(IndoorBikeDataFlags::MORE_DATA));
    }
}
