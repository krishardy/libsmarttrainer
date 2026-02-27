/// Commands that can be sent to an FTMS trainer via the Control Point.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrainerCommand {
    /// Set ERG target power in watts.
    SetTargetPower(i16),
    /// Set target resistance level (raw uint8, 0.1 resolution).
    SetTargetResistance(u8),
    /// Set target inclination (raw sint16, 0.1% resolution).
    SetTargetInclination(i16),
    /// Reset the trainer.
    Reset,
    /// Disconnect from the trainer.
    Disconnect,
}

impl TrainerCommand {
    /// Serialize this command to the raw bytes for the FTMS Control Point.
    ///
    /// Returns `None` for `Disconnect`, which is handled at the transport layer
    /// rather than being written to the control point.
    pub fn serialize(&self) -> Option<Vec<u8>> {
        match self {
            TrainerCommand::SetTargetPower(watts) => {
                Some(ftms_parser::serialize_control_point_set_target_power(*watts).to_vec())
            }
            TrainerCommand::SetTargetResistance(level) => {
                Some(ftms_parser::serialize_control_point_set_target_resistance(*level).to_vec())
            }
            TrainerCommand::SetTargetInclination(inclination) => Some(
                ftms_parser::serialize_control_point_set_target_inclination(*inclination).to_vec(),
            ),
            TrainerCommand::Reset => {
                Some(ftms_parser::serialize_control_point_reset().to_vec())
            }
            TrainerCommand::Disconnect => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialize_set_target_power() {
        let cmd = TrainerCommand::SetTargetPower(200);
        let bytes = cmd.serialize().unwrap();
        assert_eq!(bytes, vec![0x05, 0xC8, 0x00]);
    }

    #[test]
    fn serialize_set_target_power_negative() {
        let cmd = TrainerCommand::SetTargetPower(-50);
        let bytes = cmd.serialize().unwrap();
        assert_eq!(bytes[0], 0x05);
        let raw = i16::from_le_bytes([bytes[1], bytes[2]]);
        assert_eq!(raw, -50);
    }

    #[test]
    fn serialize_set_target_resistance() {
        let cmd = TrainerCommand::SetTargetResistance(50);
        let bytes = cmd.serialize().unwrap();
        assert_eq!(bytes, vec![0x04, 0x32]);
    }

    #[test]
    fn serialize_set_target_inclination() {
        let cmd = TrainerCommand::SetTargetInclination(30);
        let bytes = cmd.serialize().unwrap();
        assert_eq!(bytes, vec![0x03, 0x1E, 0x00]);
    }

    #[test]
    fn serialize_set_target_inclination_negative() {
        let cmd = TrainerCommand::SetTargetInclination(-20);
        let bytes = cmd.serialize().unwrap();
        assert_eq!(bytes[0], 0x03);
        let raw = i16::from_le_bytes([bytes[1], bytes[2]]);
        assert_eq!(raw, -20);
    }

    #[test]
    fn serialize_reset() {
        let cmd = TrainerCommand::Reset;
        let bytes = cmd.serialize().unwrap();
        assert_eq!(bytes, vec![0x01]);
    }

    #[test]
    fn serialize_disconnect_returns_none() {
        let cmd = TrainerCommand::Disconnect;
        assert!(cmd.serialize().is_none());
    }

    #[test]
    fn trainer_command_debug() {
        let cmd = TrainerCommand::SetTargetPower(100);
        let debug = format!("{:?}", cmd);
        assert!(debug.contains("SetTargetPower"));
        assert!(debug.contains("100"));
    }

    #[test]
    fn trainer_command_equality() {
        assert_eq!(
            TrainerCommand::SetTargetPower(200),
            TrainerCommand::SetTargetPower(200)
        );
        assert_ne!(
            TrainerCommand::SetTargetPower(200),
            TrainerCommand::SetTargetPower(100)
        );
        assert_ne!(TrainerCommand::Reset, TrainerCommand::Disconnect);
    }
}
