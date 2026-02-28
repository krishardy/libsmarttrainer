# trainer-quirks Engineering Decisions

## Separate crate for device-specific quirks

Device-specific behavioral interpretation is separated from the wire-protocol parser (`ftms-parser`). The parser faithfully decodes FTMS payloads per spec, while `trainer-quirks` applies post-processing for known device behaviors. This keeps `ftms-parser` as a pure `no_std` spec-compliant parser with no device-specific logic.

## Default quirks applied as a single function

`apply_default_quirks()` applies all known quirks in one call. Currently this is just HR=0 filtering (JetBlack Volt V2), but the function can be extended as new trainer quirks are discovered. The function takes `&mut IndoorBikeData` for zero-allocation in-place modification.

## no_std compatible

The crate is `no_std` to match `ftms-parser`, keeping the dependency chain embeddable. No heap allocation is needed since quirks operate on the existing `IndoorBikeData` struct in place.

## HR=0 filtering rationale

The JetBlack Volt V2 reports `heart_rate_bpm=0` when fully stopped. The BLE Heart Rate Service spec (0x2A37) defines 0 as "not available", and 0 bpm is physiologically impossible. Filtering `Some(0)` to `None` prevents downstream consumers (UI, session recording, averages) from treating it as a real measurement.
