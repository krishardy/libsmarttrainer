use ftms_parser::{ParseError, parse_indoor_bike_data};

/// C-compatible representation of parsed indoor bike data.
///
/// Uses integer types to avoid float ABI differences across architectures.
/// Speed is in units of 0.01 km/h, cadence in units of 0.5 rpm.
/// A value of `i32::MIN` indicates the field is not present.
#[repr(C)]
pub struct FfiBikeData {
    /// Speed in units of 0.01 km/h, or `i32::MIN` if absent.
    pub speed_hundredths_kmh: i32,
    /// Cadence in units of 0.5 rpm, or `i32::MIN` if absent.
    pub cadence_half_rpm: i32,
    /// Power in watts, or `i32::MIN` if absent.
    pub power_watts: i32,
    /// Heart rate in bpm, or `i32::MIN` if absent.
    pub heart_rate_bpm: i32,
}

const ABSENT: i32 = i32::MIN;

/// FFI return codes.
const FFI_OK: i32 = 0;
const FFI_ERR_NULL_POINTER: i32 = -1;
const FFI_ERR_TOO_SHORT: i32 = -2;
const FFI_ERR_INVALID_DATA: i32 = -3;

/// Parse an Indoor Bike Data (0x2AD2) notification payload.
///
/// # Safety
///
/// - `data` must point to at least `len` readable bytes, or be null.
/// - `out` must point to a valid, writable `FfiBikeData`.
///
/// Returns 0 on success, negative on error.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ftms_parse_indoor_bike_data(
    data: *const u8,
    len: usize,
    out: *mut FfiBikeData,
) -> i32 {
    if data.is_null() || out.is_null() {
        return FFI_ERR_NULL_POINTER;
    }

    let slice = unsafe { core::slice::from_raw_parts(data, len) };
    match parse_indoor_bike_data(slice) {
        Ok(parsed) => {
            let result = FfiBikeData {
                speed_hundredths_kmh: parsed
                    .instantaneous_speed_kmh
                    .map(|v| (v * 100.0) as i32)
                    .unwrap_or(ABSENT),
                cadence_half_rpm: parsed
                    .instantaneous_cadence_rpm
                    .map(|v| (v * 2.0) as i32)
                    .unwrap_or(ABSENT),
                power_watts: parsed
                    .instantaneous_power_watts
                    .map(i32::from)
                    .unwrap_or(ABSENT),
                heart_rate_bpm: parsed
                    .heart_rate_bpm
                    .map(i32::from)
                    .unwrap_or(ABSENT),
            };
            unsafe { *out = result };
            FFI_OK
        }
        Err(ParseError::TooShort) => FFI_ERR_TOO_SHORT,
        Err(ParseError::InvalidData) => FFI_ERR_INVALID_DATA,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::MaybeUninit;

    #[test]
    fn null_data_pointer() {
        let mut out = MaybeUninit::<FfiBikeData>::uninit();
        let result =
            unsafe { ftms_parse_indoor_bike_data(core::ptr::null(), 0, out.as_mut_ptr()) };
        assert_eq!(result, FFI_ERR_NULL_POINTER);
    }

    #[test]
    fn null_out_pointer() {
        let data = [0x00u8; 4];
        let result =
            unsafe { ftms_parse_indoor_bike_data(data.as_ptr(), data.len(), core::ptr::null_mut()) };
        assert_eq!(result, FFI_ERR_NULL_POINTER);
    }

    #[test]
    fn too_short_data() {
        let data = [0x00u8; 1];
        let mut out = MaybeUninit::<FfiBikeData>::uninit();
        let result =
            unsafe { ftms_parse_indoor_bike_data(data.as_ptr(), data.len(), out.as_mut_ptr()) };
        assert_eq!(result, FFI_ERR_TOO_SHORT);
    }

    #[test]
    fn valid_parse_speed_only() {
        // Flags: 0x0000 (speed present), speed = 2500 => 25.00 km/h
        let data: [u8; 4] = [0x00, 0x00, 0xC4, 0x09];
        let mut out = MaybeUninit::<FfiBikeData>::uninit();
        let result =
            unsafe { ftms_parse_indoor_bike_data(data.as_ptr(), data.len(), out.as_mut_ptr()) };
        assert_eq!(result, FFI_OK);
        let out = unsafe { out.assume_init() };
        assert_eq!(out.speed_hundredths_kmh, 2500);
        assert_eq!(out.cadence_half_rpm, ABSENT);
        assert_eq!(out.power_watts, ABSENT);
        assert_eq!(out.heart_rate_bpm, ABSENT);
    }
}
