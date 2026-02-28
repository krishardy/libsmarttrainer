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

bitflags! {
    /// Feature bits from the Fitness Machine Feature characteristic (0x2ACC),
    /// first 4 bytes (Fitness Machine Features field).
    ///
    /// Each bit indicates that the fitness machine supports the corresponding
    /// measurement or capability.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct FitnessMachineFeatures: u32 {
        const AVERAGE_SPEED            = 1 << 0;
        const CADENCE                  = 1 << 1;
        const TOTAL_DISTANCE           = 1 << 2;
        const INCLINATION              = 1 << 3;
        const ELEVATION_GAIN           = 1 << 4;
        const PACE                     = 1 << 5;
        const STEP_COUNT               = 1 << 6;
        const RESISTANCE_LEVEL         = 1 << 7;
        const STRIDE_COUNT             = 1 << 8;
        const EXPENDED_ENERGY          = 1 << 9;
        const HEART_RATE_MEASUREMENT   = 1 << 10;
        const METABOLIC_EQUIVALENT     = 1 << 11;
        const ELAPSED_TIME             = 1 << 12;
        const REMAINING_TIME           = 1 << 13;
        const POWER_MEASUREMENT        = 1 << 14;
        const FORCE_ON_BELT_AND_POWER  = 1 << 15;
        const USER_DATA_RETENTION      = 1 << 16;
    }
}

bitflags! {
    /// Target Setting Features from the Fitness Machine Feature characteristic
    /// (0x2ACC), bytes 4–7.
    ///
    /// Each bit indicates that the fitness machine supports the corresponding
    /// target-setting command.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct TargetSettingFeatures: u32 {
        const SPEED_TARGET                  = 1 << 0;
        const INCLINATION_TARGET            = 1 << 1;
        const RESISTANCE_TARGET             = 1 << 2;
        const POWER_TARGET                  = 1 << 3;
        const HEART_RATE_TARGET             = 1 << 4;
        const TARGETED_EXPENDED_ENERGY      = 1 << 5;
        const TARGETED_STEP_NUMBER          = 1 << 6;
        const TARGETED_STRIDE_NUMBER        = 1 << 7;
        const TARGETED_DISTANCE             = 1 << 8;
        const TARGETED_TRAINING_TIME        = 1 << 9;
        const TARGETED_TWO_HR_ZONES         = 1 << 10;
        const TARGETED_THREE_HR_ZONES       = 1 << 11;
        const TARGETED_FIVE_HR_ZONES        = 1 << 12;
        const INDOOR_BIKE_SIMULATION        = 1 << 13;
        const WHEEL_CIRCUMFERENCE           = 1 << 14;
        const SPIN_DOWN_CONTROL             = 1 << 15;
        const TARGETED_CADENCE              = 1 << 16;
    }
}

/// Parsed Fitness Machine Feature characteristic (0x2ACC).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FitnessMachineFeature {
    pub fitness_machine: FitnessMachineFeatures,
    pub target_setting: TargetSettingFeatures,
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

impl core::fmt::Display for ParseError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ParseError::TooShort => write!(f, "data payload too short"),
            ParseError::InvalidData => write!(f, "invalid or unsupported data format"),
        }
    }
}

/// FTMS Control Point op codes (written to 0x2AD9).
pub mod control_point {
    pub const REQUEST_CONTROL: u8 = 0x00;
    pub const RESET: u8 = 0x01;
    pub const SET_TARGET_RESISTANCE: u8 = 0x04;
    pub const SET_TARGET_POWER: u8 = 0x05;
    pub const SET_INDOOR_BIKE_SIMULATION: u8 = 0x11;
    pub const RESPONSE_CODE: u8 = 0x80;
}

/// Result code from a Control Point indication response.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlPointResultCode {
    Success,
    NotSupported,
    InvalidParameter,
    OperationFailed,
    Unknown(u8),
}

/// Parsed Control Point indication response.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ControlPointResponse {
    pub request_op_code: u8,
    pub result_code: ControlPointResultCode,
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
    [control_point::REQUEST_CONTROL]
}

/// Serialize a Fitness Machine Control Point "Reset" command (op code 0x01).
pub fn serialize_control_point_reset() -> [u8; 1] {
    [control_point::RESET]
}

/// Serialize a Fitness Machine Control Point "Set Target Power" command (op code
/// 0x05).
///
/// `watts` is the target power in watts (sint16, 1 W resolution).
pub fn serialize_control_point_set_target_power(watts: i16) -> [u8; 3] {
    let bytes = watts.to_le_bytes();
    [control_point::SET_TARGET_POWER, bytes[0], bytes[1]]
}

/// Serialize a Fitness Machine Control Point "Set Target Resistance Level"
/// command (op code 0x04).
///
/// `level` is the raw uint8 value with 0.1 resolution (caller applies scaling).
pub fn serialize_control_point_set_target_resistance(level: u8) -> [u8; 2] {
    [control_point::SET_TARGET_RESISTANCE, level]
}

/// Serialize a Fitness Machine Control Point "Set Indoor Bike Simulation"
/// command (op code 0x11).
///
/// Parameters:
/// - `wind_speed_001_mps`: wind speed in 0.001 m/s resolution (sint16)
/// - `grade_001_pct`: grade in 0.01% resolution (sint16)
/// - `crr_0001`: rolling resistance coefficient in 0.0001 resolution (uint8)
/// - `cw_001_kgm`: wind resistance coefficient in 0.01 kg/m resolution (uint8)
pub fn serialize_control_point_set_indoor_bike_simulation(
    wind_speed_001_mps: i16,
    grade_001_pct: i16,
    crr_0001: u8,
    cw_001_kgm: u8,
) -> [u8; 7] {
    let wind = wind_speed_001_mps.to_le_bytes();
    let grade = grade_001_pct.to_le_bytes();
    [
        control_point::SET_INDOOR_BIKE_SIMULATION,
        wind[0],
        wind[1],
        grade[0],
        grade[1],
        crr_0001,
        cw_001_kgm,
    ]
}

/// Parse a Fitness Machine Control Point indication response.
///
/// The response format is `[0x80, <request_op_code>, <result_code>]`.
pub fn parse_control_point_response(data: &[u8]) -> Result<ControlPointResponse, ParseError> {
    if data.len() < 3 {
        return Err(ParseError::TooShort);
    }
    if data[0] != control_point::RESPONSE_CODE {
        return Err(ParseError::InvalidData);
    }
    let result_code = match data[2] {
        0x01 => ControlPointResultCode::Success,
        0x02 => ControlPointResultCode::NotSupported,
        0x03 => ControlPointResultCode::InvalidParameter,
        0x06 => ControlPointResultCode::OperationFailed,
        other => ControlPointResultCode::Unknown(other),
    };
    Ok(ControlPointResponse {
        request_op_code: data[1],
        result_code,
    })
}

/// Parse the Fitness Machine Feature characteristic (0x2ACC).
///
/// The characteristic is 8 bytes: 4 bytes for Fitness Machine Features followed
/// by 4 bytes for Target Setting Features.
pub fn parse_feature(data: &[u8]) -> Result<FitnessMachineFeature, ParseError> {
    if data.len() < 8 {
        return Err(ParseError::TooShort);
    }
    let fitness_machine = FitnessMachineFeatures::from_bits_truncate(
        u32::from_le_bytes([data[0], data[1], data[2], data[3]]),
    );
    let target_setting = TargetSettingFeatures::from_bits_truncate(
        u32::from_le_bytes([data[4], data[5], data[6], data[7]]),
    );
    Ok(FitnessMachineFeature {
        fitness_machine,
        target_setting,
    })
}

#[cfg(test)]
extern crate alloc;

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::format;

    // ── Display tests ──────────────────────────────────────────

    #[test]
    fn display_parse_error_too_short() {
        let err = ParseError::TooShort;
        assert_eq!(format!("{err}"), "data payload too short");
    }

    #[test]
    fn display_parse_error_invalid_data() {
        let err = ParseError::InvalidData;
        assert_eq!(format!("{err}"), "invalid or unsupported data format");
    }

    // ── Parser tests ──────────────────────────────────────────

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
        // Fitness Machine Features: AVERAGE_SPEED (bit 0) = 0x00000001
        // Target Setting Features: SPEED_TARGET (bit 0) = 0x00000001
        let data = [0x01, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00];
        let result = parse_feature(&data).unwrap();
        assert!(result.fitness_machine.contains(FitnessMachineFeatures::AVERAGE_SPEED));
        assert!(result.target_setting.contains(TargetSettingFeatures::SPEED_TARGET));
    }

    #[test]
    fn parse_feature_too_short() {
        assert_eq!(parse_feature(&[0x01, 0x02]), Err(ParseError::TooShort));
    }

    #[test]
    fn parse_feature_power_and_cadence() {
        // POWER_MEASUREMENT (bit 14) | CADENCE (bit 1) = 0x00004002
        let data = [0x02, 0x40, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
        let result = parse_feature(&data).unwrap();
        assert!(result.fitness_machine.contains(FitnessMachineFeatures::POWER_MEASUREMENT));
        assert!(result.fitness_machine.contains(FitnessMachineFeatures::CADENCE));
        assert!(!result.fitness_machine.contains(FitnessMachineFeatures::AVERAGE_SPEED));
    }

    #[test]
    fn parse_feature_target_settings() {
        // Target: POWER_TARGET (bit 3) | RESISTANCE_TARGET (bit 2) | INCLINATION_TARGET (bit 1) = 0x0000000E
        let data = [0x00, 0x00, 0x00, 0x00, 0x0E, 0x00, 0x00, 0x00];
        let result = parse_feature(&data).unwrap();
        assert!(result.fitness_machine.is_empty());
        assert!(result.target_setting.contains(TargetSettingFeatures::POWER_TARGET));
        assert!(result.target_setting.contains(TargetSettingFeatures::RESISTANCE_TARGET));
        assert!(result.target_setting.contains(TargetSettingFeatures::INCLINATION_TARGET));
        assert!(!result.target_setting.contains(TargetSettingFeatures::SPEED_TARGET));
    }

    #[test]
    fn parse_feature_empty_features() {
        let data = [0u8; 8];
        let result = parse_feature(&data).unwrap();
        assert!(result.fitness_machine.is_empty());
        assert!(result.target_setting.is_empty());
    }

    #[test]
    fn parse_feature_ignores_extra_bytes() {
        let mut data = [0u8; 10];
        data[0] = 0x01; // AVERAGE_SPEED
        data[4] = 0x08; // POWER_TARGET
        data[8] = 0xFF; // extra, ignored
        data[9] = 0xFF; // extra, ignored
        let result = parse_feature(&data).unwrap();
        assert!(result.fitness_machine.contains(FitnessMachineFeatures::AVERAGE_SPEED));
        assert!(result.target_setting.contains(TargetSettingFeatures::POWER_TARGET));
    }

    #[test]
    fn parse_feature_too_short_7_bytes() {
        assert_eq!(parse_feature(&[0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07]), Err(ParseError::TooShort));
    }

    #[test]
    fn feature_bitflags_round_trip() {
        let fm = FitnessMachineFeatures::CADENCE | FitnessMachineFeatures::POWER_MEASUREMENT;
        assert_eq!(fm.bits(), (1 << 1) | (1 << 14));
        assert!(fm.contains(FitnessMachineFeatures::CADENCE));
        assert!(!fm.contains(FitnessMachineFeatures::AVERAGE_SPEED));

        let ts = TargetSettingFeatures::POWER_TARGET | TargetSettingFeatures::INDOOR_BIKE_SIMULATION;
        assert_eq!(ts.bits(), (1 << 3) | (1 << 13));
        assert!(ts.contains(TargetSettingFeatures::POWER_TARGET));
        assert!(!ts.contains(TargetSettingFeatures::SPEED_TARGET));
    }

    #[test]
    fn bitflags_round_trip() {
        let flags = IndoorBikeDataFlags::INSTANTANEOUS_CADENCE
            | IndoorBikeDataFlags::INSTANTANEOUS_POWER;
        assert_eq!(flags.bits(), 0x0044);
        assert!(flags.contains(IndoorBikeDataFlags::INSTANTANEOUS_CADENCE));
        assert!(!flags.contains(IndoorBikeDataFlags::MORE_DATA));
    }

    #[test]
    fn serialize_reset() {
        assert_eq!(serialize_control_point_reset(), [0x01]);
    }

    #[test]
    fn serialize_set_target_power_200w() {
        assert_eq!(
            serialize_control_point_set_target_power(200),
            [0x05, 0xC8, 0x00]
        );
    }

    #[test]
    fn serialize_set_target_power_negative() {
        let bytes = serialize_control_point_set_target_power(-50);
        // Round-trip: parse the sint16 back out.
        let raw = i16::from_le_bytes([bytes[1], bytes[2]]);
        assert_eq!(raw, -50);
        assert_eq!(bytes[0], control_point::SET_TARGET_POWER);
    }

    #[test]
    fn serialize_set_target_resistance_5_0() {
        // 5.0 at 0.1 resolution = raw value 50 (0x32)
        assert_eq!(
            serialize_control_point_set_target_resistance(0x32),
            [0x04, 0x32]
        );
    }

    #[test]
    fn serialize_indoor_bike_simulation_zero_grade() {
        let bytes = serialize_control_point_set_indoor_bike_simulation(0, 0, 40, 51);
        assert_eq!(bytes, [0x11, 0x00, 0x00, 0x00, 0x00, 40, 51]);
    }

    #[test]
    fn serialize_indoor_bike_simulation_positive_grade() {
        // 5.0% at 0.01% resolution = 500
        let bytes = serialize_control_point_set_indoor_bike_simulation(0, 500, 40, 51);
        assert_eq!(bytes[0], 0x11);
        // wind = 0
        assert_eq!(i16::from_le_bytes([bytes[1], bytes[2]]), 0);
        // grade = 500
        assert_eq!(i16::from_le_bytes([bytes[3], bytes[4]]), 500);
        assert_eq!(bytes[5], 40);
        assert_eq!(bytes[6], 51);
    }

    #[test]
    fn serialize_indoor_bike_simulation_negative_grade() {
        // -3.0% at 0.01% resolution = -300
        let bytes = serialize_control_point_set_indoor_bike_simulation(0, -300, 40, 51);
        assert_eq!(bytes[0], control_point::SET_INDOOR_BIKE_SIMULATION);
        assert_eq!(i16::from_le_bytes([bytes[3], bytes[4]]), -300);
    }

    #[test]
    fn parse_control_point_response_success() {
        let data = [0x80, 0x00, 0x01];
        let resp = parse_control_point_response(&data).unwrap();
        assert_eq!(resp.request_op_code, control_point::REQUEST_CONTROL);
        assert_eq!(resp.result_code, ControlPointResultCode::Success);
    }

    #[test]
    fn parse_control_point_response_not_supported() {
        let data = [0x80, 0x05, 0x02];
        let resp = parse_control_point_response(&data).unwrap();
        assert_eq!(resp.request_op_code, control_point::SET_TARGET_POWER);
        assert_eq!(resp.result_code, ControlPointResultCode::NotSupported);
    }

    #[test]
    fn parse_control_point_response_too_short() {
        assert_eq!(
            parse_control_point_response(&[0x80, 0x00]),
            Err(ParseError::TooShort)
        );
    }

    #[test]
    fn parse_control_point_response_invalid_prefix() {
        assert_eq!(
            parse_control_point_response(&[0x00, 0x00, 0x01]),
            Err(ParseError::InvalidData)
        );
    }

    #[test]
    fn parse_control_point_response_invalid_parameter() {
        let data = [0x80, 0x05, 0x03];
        let resp = parse_control_point_response(&data).unwrap();
        assert_eq!(resp.request_op_code, control_point::SET_TARGET_POWER);
        assert_eq!(resp.result_code, ControlPointResultCode::InvalidParameter);
    }

    #[test]
    fn parse_control_point_response_operation_failed() {
        let data = [0x80, 0x04, 0x06];
        let resp = parse_control_point_response(&data).unwrap();
        assert_eq!(resp.request_op_code, control_point::SET_TARGET_RESISTANCE);
        assert_eq!(resp.result_code, ControlPointResultCode::OperationFailed);
    }

    #[test]
    fn parse_control_point_response_unknown_code() {
        let data = [0x80, 0x01, 0xFF];
        let resp = parse_control_point_response(&data).unwrap();
        assert_eq!(resp.request_op_code, control_point::RESET);
        assert_eq!(resp.result_code, ControlPointResultCode::Unknown(0xFF));
    }

    #[test]
    fn parse_indoor_bike_data_truncated_avg_speed() {
        // AVERAGE_SPEED flag set but no data for it after speed bytes.
        // Flags: AVERAGE_SPEED = 0x0002, MORE_DATA not set => speed present.
        // Provide flags + speed only, no avg speed bytes.
        let data = [0x02, 0x00, 0xC4, 0x09];
        assert_eq!(parse_indoor_bike_data(&data), Err(ParseError::TooShort));
    }

    #[test]
    fn parse_indoor_bike_data_truncated_cadence() {
        // INSTANTANEOUS_CADENCE flag set but payload ends before cadence bytes.
        // Flags: INSTANTANEOUS_CADENCE = 0x0004.
        let data = [0x04, 0x00, 0xC4, 0x09];
        assert_eq!(parse_indoor_bike_data(&data), Err(ParseError::TooShort));
    }

    #[test]
    fn parse_indoor_bike_data_truncated_avg_cadence() {
        // INSTANTANEOUS_CADENCE | AVERAGE_CADENCE = 0x000C.
        // Provide speed + cadence but no avg cadence.
        let data = [0x0C, 0x00, 0xC4, 0x09, 0xB4, 0x00];
        assert_eq!(parse_indoor_bike_data(&data), Err(ParseError::TooShort));
    }

    #[test]
    fn parse_indoor_bike_data_truncated_total_distance() {
        // TOTAL_DISTANCE = 0x0010.
        let data = [0x10, 0x00, 0xC4, 0x09];
        assert_eq!(parse_indoor_bike_data(&data), Err(ParseError::TooShort));
    }

    #[test]
    fn parse_indoor_bike_data_truncated_resistance() {
        // RESISTANCE_LEVEL = 0x0020.
        let data = [0x20, 0x00, 0xC4, 0x09];
        assert_eq!(parse_indoor_bike_data(&data), Err(ParseError::TooShort));
    }

    #[test]
    fn parse_indoor_bike_data_truncated_power() {
        // INSTANTANEOUS_POWER = 0x0040.
        let data = [0x40, 0x00, 0xC4, 0x09];
        assert_eq!(parse_indoor_bike_data(&data), Err(ParseError::TooShort));
    }

    #[test]
    fn parse_indoor_bike_data_truncated_avg_power() {
        // INSTANTANEOUS_POWER | AVERAGE_POWER = 0x00C0.
        // Provide speed + power but no avg power.
        let data = [0xC0, 0x00, 0xC4, 0x09, 0xC8, 0x00];
        assert_eq!(parse_indoor_bike_data(&data), Err(ParseError::TooShort));
    }

    #[test]
    fn parse_indoor_bike_data_truncated_expended_energy() {
        // EXPENDED_ENERGY = 0x0100.
        let data = [0x00, 0x01, 0xC4, 0x09];
        assert_eq!(parse_indoor_bike_data(&data), Err(ParseError::TooShort));
    }

    #[test]
    fn parse_indoor_bike_data_truncated_heart_rate() {
        // HEART_RATE = 0x0200.
        let data = [0x00, 0x02, 0xC4, 0x09];
        assert_eq!(parse_indoor_bike_data(&data), Err(ParseError::TooShort));
    }

    #[test]
    fn parse_feature_typical_indoor_bike_trainer() {
        // Fitness Machine Features: CADENCE(1) | RESISTANCE_LEVEL(7) | ELAPSED_TIME(12) | POWER_MEASUREMENT(14) = 0x00005082
        // Target Setting Features: RESISTANCE_TARGET(2) | POWER_TARGET(3) | INDOOR_BIKE_SIMULATION(13) = 0x0000200C
        let data = [0x82, 0x50, 0x00, 0x00, 0x0C, 0x20, 0x00, 0x00];
        let result = parse_feature(&data).unwrap();
        assert!(result.fitness_machine.contains(FitnessMachineFeatures::CADENCE));
        assert!(result.fitness_machine.contains(FitnessMachineFeatures::RESISTANCE_LEVEL));
        assert!(result.fitness_machine.contains(FitnessMachineFeatures::ELAPSED_TIME));
        assert!(result.fitness_machine.contains(FitnessMachineFeatures::POWER_MEASUREMENT));
        assert!(!result.fitness_machine.contains(FitnessMachineFeatures::AVERAGE_SPEED));
        assert!(!result.fitness_machine.contains(FitnessMachineFeatures::HEART_RATE_MEASUREMENT));
        assert!(result.target_setting.contains(TargetSettingFeatures::RESISTANCE_TARGET));
        assert!(result.target_setting.contains(TargetSettingFeatures::POWER_TARGET));
        assert!(result.target_setting.contains(TargetSettingFeatures::INDOOR_BIKE_SIMULATION));
        assert!(!result.target_setting.contains(TargetSettingFeatures::SPEED_TARGET));
        assert!(!result.target_setting.contains(TargetSettingFeatures::INCLINATION_TARGET));
    }

    #[test]
    fn parse_indoor_bike_data_speed_and_heart_rate() {
        // Flags: HEART_RATE = 0x0200, MORE_DATA not set => speed present.
        // Speed: 2800 => 28.00 km/h, HR: 145 bpm
        let data = [0x00, 0x02, 0xF0, 0x0A, 0x91];
        let result = parse_indoor_bike_data(&data).unwrap();
        assert!((result.instantaneous_speed_kmh.unwrap() - 28.0).abs() < 0.01);
        assert_eq!(result.instantaneous_cadence_rpm, None);
        assert_eq!(result.instantaneous_power_watts, None);
        assert_eq!(result.heart_rate_bpm, Some(145));
    }

    #[test]
    fn parse_indoor_bike_data_zero_speed() {
        // Flags: 0x0000, Speed: 0 => 0.00 km/h
        let data = [0x00, 0x00, 0x00, 0x00];
        let result = parse_indoor_bike_data(&data).unwrap();
        assert!((result.instantaneous_speed_kmh.unwrap()).abs() < 0.01);
        assert_eq!(result.instantaneous_cadence_rpm, None);
        assert_eq!(result.instantaneous_power_watts, None);
        assert_eq!(result.heart_rate_bpm, None);
    }

    #[test]
    fn parse_indoor_bike_data_high_cadence_high_power() {
        // Flags: INSTANTANEOUS_CADENCE | INSTANTANEOUS_POWER = 0x0044
        // Speed: 4500 => 45.00 km/h, Cadence: 240 => 120.0 rpm, Power: 400W
        let data = [0x44, 0x00, 0x94, 0x11, 0xF0, 0x00, 0x90, 0x01];
        let result = parse_indoor_bike_data(&data).unwrap();
        assert!((result.instantaneous_speed_kmh.unwrap() - 45.0).abs() < 0.01);
        assert!((result.instantaneous_cadence_rpm.unwrap() - 120.0).abs() < 0.1);
        assert_eq!(result.instantaneous_power_watts, Some(400));
    }

    #[test]
    fn parse_indoor_bike_data_negative_power() {
        // Flags: INSTANTANEOUS_POWER = 0x0040
        // Speed: 1000 => 10.00 km/h, Power: -10W (0xFFF6 as i16 LE)
        let data = [0x40, 0x00, 0xE8, 0x03, 0xF6, 0xFF];
        let result = parse_indoor_bike_data(&data).unwrap();
        assert!((result.instantaneous_speed_kmh.unwrap() - 10.0).abs() < 0.01);
        assert_eq!(result.instantaneous_power_watts, Some(-10));
    }

    #[test]
    fn parse_indoor_bike_data_more_data_power_only() {
        // Flags: MORE_DATA | INSTANTANEOUS_POWER = 0x0041
        // No speed bytes (MORE_DATA set), Power: 150W
        let data = [0x41, 0x00, 0x96, 0x00];
        let result = parse_indoor_bike_data(&data).unwrap();
        assert_eq!(result.instantaneous_speed_kmh, None);
        assert_eq!(result.instantaneous_cadence_rpm, None);
        assert_eq!(result.instantaneous_power_watts, Some(150));
        assert_eq!(result.heart_rate_bpm, None);
    }

    #[test]
    fn parse_control_point_response_extra_bytes() {
        // Some trainers send trailing padding bytes; parser should ignore them.
        let data = [0x80, 0x05, 0x01, 0x00, 0x00];
        let resp = parse_control_point_response(&data).unwrap();
        assert_eq!(resp.request_op_code, control_point::SET_TARGET_POWER);
        assert_eq!(resp.result_code, ControlPointResultCode::Success);
    }

    #[test]
    fn serialize_set_target_resistance_zero() {
        assert_eq!(
            serialize_control_point_set_target_resistance(0),
            [0x04, 0x00]
        );
    }

    #[test]
    fn serialize_set_target_power_zero() {
        assert_eq!(
            serialize_control_point_set_target_power(0),
            [0x05, 0x00, 0x00]
        );
    }


    // ---------------------------------------------------------------
    // Real-device test vectors: JetBlack Volt V2
    // Captured via btmon + bluetoothctl on Linux, 2026-02-27.
    // Trainer BLE address: E1:A2:F9:12:CF:38
    // All Indoor Bike Data payloads use flags 0x0264:
    //   speed + cadence + resistance_level + power + heart_rate
    // ---------------------------------------------------------------

    #[test]
    fn real_jetblack_volt_v2_feature() {
        // Captured from JetBlack Volt V2 via btmon
        // 0x2ACC Fitness Machine Feature characteristic — ATT Read Response
        let data = [0x87, 0x44, 0x00, 0x00, 0x0c, 0xe0, 0x00, 0x00];
        let result = parse_feature(&data).unwrap();
        // Fitness Machine Features: 0x00004487
        assert!(result.fitness_machine.contains(FitnessMachineFeatures::AVERAGE_SPEED));
        assert!(result.fitness_machine.contains(FitnessMachineFeatures::CADENCE));
        assert!(result.fitness_machine.contains(FitnessMachineFeatures::TOTAL_DISTANCE));
        assert!(result.fitness_machine.contains(FitnessMachineFeatures::RESISTANCE_LEVEL));
        assert!(result.fitness_machine.contains(FitnessMachineFeatures::HEART_RATE_MEASUREMENT));
        assert!(result.fitness_machine.contains(FitnessMachineFeatures::POWER_MEASUREMENT));
        assert!(!result.fitness_machine.contains(FitnessMachineFeatures::INCLINATION));
        assert!(!result.fitness_machine.contains(FitnessMachineFeatures::ELAPSED_TIME));
        // Target Setting Features: 0x0000e00c
        assert!(result.target_setting.contains(TargetSettingFeatures::RESISTANCE_TARGET));
        assert!(result.target_setting.contains(TargetSettingFeatures::POWER_TARGET));
        assert!(result.target_setting.contains(TargetSettingFeatures::INDOOR_BIKE_SIMULATION));
        assert!(result.target_setting.contains(TargetSettingFeatures::WHEEL_CIRCUMFERENCE));
        assert!(result.target_setting.contains(TargetSettingFeatures::SPIN_DOWN_CONTROL));
        assert!(!result.target_setting.contains(TargetSettingFeatures::SPEED_TARGET));
        assert!(!result.target_setting.contains(TargetSettingFeatures::INCLINATION_TARGET));
    }

    #[test]
    fn real_jetblack_volt_v2_indoor_bike_data_at_rest() {
        // Captured from JetBlack Volt V2 via btmon — trainer idle, not pedaling
        // 0x2AD2 Indoor Bike Data notification (t≈110s in capture)
        let data = [0x64, 0x02, 0x00, 0x00, 0x00, 0x00, 0x04, 0x00, 0x00, 0x00, 0x4f];
        let result = parse_indoor_bike_data(&data).unwrap();
        assert!((result.instantaneous_speed_kmh.unwrap()).abs() < 0.01);
        assert!((result.instantaneous_cadence_rpm.unwrap()).abs() < 0.1);
        assert_eq!(result.instantaneous_power_watts, Some(0));
        assert_eq!(result.heart_rate_bpm, Some(79));
    }

    #[test]
    fn real_jetblack_volt_v2_indoor_bike_data_easy_spin() {
        // Captured from JetBlack Volt V2 via btmon — light pedaling
        // 0x2AD2 Indoor Bike Data notification (t≈143s in capture)
        let data = [0x64, 0x02, 0xb2, 0x06, 0x7c, 0x00, 0x04, 0x00, 0x3a, 0x00, 0x56];
        let result = parse_indoor_bike_data(&data).unwrap();
        assert!((result.instantaneous_speed_kmh.unwrap() - 17.14).abs() < 0.01);
        assert!((result.instantaneous_cadence_rpm.unwrap() - 62.0).abs() < 0.1);
        assert_eq!(result.instantaneous_power_watts, Some(58));
        assert_eq!(result.heart_rate_bpm, Some(86));
    }

    #[test]
    fn real_jetblack_volt_v2_indoor_bike_data_moderate_effort() {
        // Captured from JetBlack Volt V2 via btmon — steady moderate pedaling
        // 0x2AD2 Indoor Bike Data notification (t≈165s in capture)
        let data = [0x64, 0x02, 0xd3, 0x0a, 0x90, 0x00, 0x04, 0x00, 0xa2, 0x00, 0x63];
        let result = parse_indoor_bike_data(&data).unwrap();
        assert!((result.instantaneous_speed_kmh.unwrap() - 27.71).abs() < 0.01);
        assert!((result.instantaneous_cadence_rpm.unwrap() - 72.0).abs() < 0.1);
        assert_eq!(result.instantaneous_power_watts, Some(162));
        assert_eq!(result.heart_rate_bpm, Some(99));
    }

    #[test]
    fn real_jetblack_volt_v2_indoor_bike_data_sprint() {
        // Captured from JetBlack Volt V2 via btmon — peak sprint effort
        // 0x2AD2 Indoor Bike Data notification (t≈175s in capture)
        let data = [0x64, 0x02, 0x42, 0x0f, 0x9e, 0x00, 0x04, 0x00, 0x22, 0x02, 0x6b];
        let result = parse_indoor_bike_data(&data).unwrap();
        assert!((result.instantaneous_speed_kmh.unwrap() - 39.06).abs() < 0.01);
        assert!((result.instantaneous_cadence_rpm.unwrap() - 79.0).abs() < 0.1);
        assert_eq!(result.instantaneous_power_watts, Some(546));
        assert_eq!(result.heart_rate_bpm, Some(107));
    }

    #[test]
    fn real_jetblack_volt_v2_indoor_bike_data_coast_down() {
        // Captured from JetBlack Volt V2 via btmon — stopped pedaling, speed decreasing
        // 0x2AD2 Indoor Bike Data notification (t≈199s in capture)
        let data = [0x64, 0x02, 0x30, 0x03, 0x00, 0x00, 0x04, 0x00, 0x00, 0x00, 0x76];
        let result = parse_indoor_bike_data(&data).unwrap();
        assert!((result.instantaneous_speed_kmh.unwrap() - 8.16).abs() < 0.01);
        assert!((result.instantaneous_cadence_rpm.unwrap()).abs() < 0.1);
        assert_eq!(result.instantaneous_power_watts, Some(0));
        assert_eq!(result.heart_rate_bpm, Some(118));
    }

    #[test]
    fn real_jetblack_volt_v2_indoor_bike_data_stopped_hr_zero() {
        // Captured from JetBlack Volt V2 via btmon — fully stopped, HR reports 0
        // 0x2AD2 Indoor Bike Data notification (t≈207s in capture)
        // Demonstrates trainer quirk: heart_rate_bpm=0 when fully stopped.
        let data = [0x64, 0x02, 0x00, 0x00, 0x00, 0x00, 0x04, 0x00, 0x00, 0x00, 0x00];
        let result = parse_indoor_bike_data(&data).unwrap();
        assert!((result.instantaneous_speed_kmh.unwrap()).abs() < 0.01);
        assert!((result.instantaneous_cadence_rpm.unwrap()).abs() < 0.1);
        assert_eq!(result.instantaneous_power_watts, Some(0));
        assert_eq!(result.heart_rate_bpm, Some(0));
    }

    #[test]
    fn parse_indoor_bike_data_all_fields() {
        // Flags: MORE_DATA | AVERAGE_SPEED | INSTANTANEOUS_CADENCE |
        //        AVERAGE_CADENCE | TOTAL_DISTANCE | RESISTANCE_LEVEL |
        //        INSTANTANEOUS_POWER | AVERAGE_POWER | EXPENDED_ENERGY |
        //        HEART_RATE = 0x03FF
        //
        // Layout after 2-byte flags:
        //   avg speed (2), inst cadence (2), avg cadence (2),
        //   total distance (3), resistance level (2), inst power (2),
        //   avg power (2), expended energy (5), heart rate (1)
        let data: [u8; 23] = [
            0xFF, 0x03, // flags
            0x00, 0x00, // avg speed (skipped)
            0xB4, 0x00, // inst cadence: 180 => 90.0 rpm
            0x00, 0x00, // avg cadence (skipped)
            0x00, 0x00, 0x00, // total distance (skipped)
            0x00, 0x00, // resistance level (skipped)
            0xC8, 0x00, // inst power: 200W
            0x00, 0x00, // avg power (skipped)
            0x00, 0x00, 0x00, 0x00, 0x00, // expended energy (skipped)
            0x48, // heart rate: 72 bpm
        ];
        let result = parse_indoor_bike_data(&data).unwrap();
        // MORE_DATA set => no speed
        assert_eq!(result.instantaneous_speed_kmh, None);
        assert!((result.instantaneous_cadence_rpm.unwrap() - 90.0).abs() < 0.1);
        assert_eq!(result.instantaneous_power_watts, Some(200));
        assert_eq!(result.heart_rate_bpm, Some(72));
    }
}
