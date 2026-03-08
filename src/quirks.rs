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

use crate::parser::IndoorBikeData;

/// Apply default trainer quirks to parsed Indoor Bike Data.
///
/// Currently handles:
/// - **HR=0 filtering**: Some trainers (e.g., JetBlack Volt V2) report
///   `heart_rate_bpm=0` when fully stopped. Since 0 bpm is physiologically
///   impossible (the BLE HR Service spec 0x2A37 uses 0 to mean "not
///   available"), this is mapped to `None`.
pub fn apply_default_quirks(data: &mut IndoorBikeData) {
    if data.heart_rate_bpm == Some(0) {
        data.heart_rate_bpm = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_bike_data() -> IndoorBikeData {
        IndoorBikeData {
            instantaneous_speed_kmh: Some(25.0),
            instantaneous_cadence_rpm: Some(90.0),
            instantaneous_power_watts: Some(200),
            heart_rate_bpm: None,
        }
    }

    #[test]
    fn hr_zero_filtered() {
        let mut data = base_bike_data();
        data.heart_rate_bpm = Some(0);
        apply_default_quirks(&mut data);
        assert_eq!(data.heart_rate_bpm, None);
    }

    #[test]
    fn hr_nonzero_preserved() {
        let mut data = base_bike_data();
        data.heart_rate_bpm = Some(145);
        apply_default_quirks(&mut data);
        assert_eq!(data.heart_rate_bpm, Some(145));
    }

    #[test]
    fn hr_one_preserved() {
        let mut data = base_bike_data();
        data.heart_rate_bpm = Some(1);
        apply_default_quirks(&mut data);
        assert_eq!(data.heart_rate_bpm, Some(1));
    }

    #[test]
    fn hr_none_stays_none() {
        let mut data = base_bike_data();
        assert_eq!(data.heart_rate_bpm, None);
        apply_default_quirks(&mut data);
        assert_eq!(data.heart_rate_bpm, None);
    }

    #[test]
    fn other_fields_untouched() {
        let mut data = base_bike_data();
        data.heart_rate_bpm = Some(0);
        apply_default_quirks(&mut data);
        assert!((data.instantaneous_speed_kmh.unwrap() - 25.0).abs() < 0.01);
        assert!((data.instantaneous_cadence_rpm.unwrap() - 90.0).abs() < 0.1);
        assert_eq!(data.instantaneous_power_watts, Some(200));
    }

    #[test]
    fn max_hr_preserved() {
        let mut data = base_bike_data();
        data.heart_rate_bpm = Some(255);
        apply_default_quirks(&mut data);
        assert_eq!(data.heart_rate_bpm, Some(255));
    }
}
