# TODOs

## TODO

### Hardening

- [ ] Multiple trainer support (handle >1 FTMS device in range) [Low, 2h]
- [ ] Trainer-specific quirk testing and workarounds [4.7, Med, 4h]
- [ ] Move any JetBlack Volt v2 code to a workspace member crate [3]

### Documention

- [ ] Create library usage examples [2]
      - Scanning
      - Connecting
      - Reading data
      - Writing data

## Done

### Hardening

- [x] Graceful error types: PermissionDenied variant, Display for ParseError, user_message() with recovery hints on BleTransportError [4.4]

### BLE Transport (ble-transport crate)

- [x] Automatic reconnection logic on BLE drop with exponential backoff, address matching, and control command restoration [2.9]
- [x] ERG death spiral protection: suspend ERG when cadence < 40 RPM for 3s, ramp from 0 to target over 15s after recovery to 85 RPM [2.9a]

### BLE Transport (ble-transport crate) — earlier

- [x] Build ble-transport crate: async scan/connect/subscribe with tokio::sync::watch channel API [2.1, High, 6h]
- [x] Connection state tracking (connecting/connected/disconnected state machine) [2.8, Med, 1h]

### FTMS Protocol (ftms-parser crate)

- [x] Implement Indoor Bike Data parser (0x2AD2): flags bitfield walking, speed/cadence/power extraction [1.2, High, 4h]
- [x] Implement Control Point command serializer (0x2AD9): request control, set target power, resistance, grade [1.3, High, 2h]
- [x] Implement Feature characteristic parser (0x2ACC) [1.4, Med, 1h]
- [x] Unit tests with hardcoded byte payloads for all parser functions [1.5, High, 2h]
- [x] Capture real notification payloads and add as test vectors [1.8, Med, 1h]

### Workspace & Structure

- [x] Set up Rust workspace with crate structure (ftms-parser, ble-transport) [1.1, High, 2h]
