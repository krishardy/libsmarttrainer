use uuid::Uuid;

/// FTMS Service UUID (0x1826).
pub const FTMS_SERVICE_UUID: Uuid = Uuid::from_u128(0x0000_1826_0000_1000_8000_0080_5f9b_34fb);

/// Indoor Bike Data characteristic UUID (0x2AD2).
pub const INDOOR_BIKE_DATA_UUID: Uuid =
    Uuid::from_u128(0x0000_2ad2_0000_1000_8000_0080_5f9b_34fb);

/// Fitness Machine Feature characteristic UUID (0x2ACC).
pub const FEATURE_UUID: Uuid = Uuid::from_u128(0x0000_2acc_0000_1000_8000_0080_5f9b_34fb);

/// Fitness Machine Control Point characteristic UUID (0x2AD9).
pub const CONTROL_POINT_UUID: Uuid =
    Uuid::from_u128(0x0000_2ad9_0000_1000_8000_0080_5f9b_34fb);

/// Fitness Machine Status characteristic UUID (0x2ADA).
pub const FITNESS_MACHINE_STATUS_UUID: Uuid =
    Uuid::from_u128(0x0000_2ada_0000_1000_8000_0080_5f9b_34fb);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ftms_service_uuid_value() {
        assert_eq!(
            FTMS_SERVICE_UUID.to_string(),
            "00001826-0000-1000-8000-00805f9b34fb"
        );
    }

    #[test]
    fn indoor_bike_data_uuid_value() {
        assert_eq!(
            INDOOR_BIKE_DATA_UUID.to_string(),
            "00002ad2-0000-1000-8000-00805f9b34fb"
        );
    }

    #[test]
    fn feature_uuid_value() {
        assert_eq!(
            FEATURE_UUID.to_string(),
            "00002acc-0000-1000-8000-00805f9b34fb"
        );
    }

    #[test]
    fn control_point_uuid_value() {
        assert_eq!(
            CONTROL_POINT_UUID.to_string(),
            "00002ad9-0000-1000-8000-00805f9b34fb"
        );
    }

    #[test]
    fn fitness_machine_status_uuid_value() {
        assert_eq!(
            FITNESS_MACHINE_STATUS_UUID.to_string(),
            "00002ada-0000-1000-8000-00805f9b34fb"
        );
    }
}
