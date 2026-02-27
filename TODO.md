# TODOs

## TODO

### FTMS Protocol (ftms-parser crate)

- [ ] Capture real notification payloads and add as test vectors [1.8, Med, 1h]

### BLE Transport (ble-transport crate)

- [ ] Build ble-transport crate: async scan/connect/subscribe with tokio::sync::watch channel API [2.1, High, 6h]
- [ ] Connection state tracking (connecting/connected/disconnected state machine) [2.8, Med, 1h]
- [ ] Automatic reconnection logic on BLE drop [2.9, Med, 3h]

### iOS FFI (ftms-parser-ffi crate)

- [ ] ftms-parser-ffi: extern "C" functions + cbindgen header generation [3.1, High, 3h]
- [ ] Cross-compile for aarch64-apple-ios and aarch64-apple-ios-sim targets [3.2, High, 2h]
- [ ] Wire notification bytes through Rust FFI parser [3.5, High, 2h]

### iOS BLE (Swift companion module)

- [ ] CoreBluetooth manager class: scan for 0x1826, connect, discover, subscribe [3.4, High, 6h]
- [ ] Background BLE: bluetooth-central mode, state restoration [3.10, Med, 3h]
- [ ] Reconnection handling (iOS) [3.11, Med, 2h]

### Hardening

- [ ] Graceful error types: BLE permission denied, trainer not found, characteristic missing [4.4, High, 2h]
- [ ] Multiple trainer support (handle >1 FTMS device in range) [4.5, Low, 2h]
- [ ] Trainer-specific quirk testing and workarounds [4.7, Med, 4h]

## Done


### FTMS Protocol (ftms-parser crate)

- [x] Implement Indoor Bike Data parser (0x2AD2): flags bitfield walking, speed/cadence/power extraction [1.2, High, 4h]
- [x] Implement Control Point command serializer (0x2AD9): request control, set target power, resistance, grade [1.3, High, 2h]
- [x] Implement Feature characteristic parser (0x2ACC) [1.4, Med, 1h]
- [x] Unit tests with hardcoded byte payloads for all parser functions [1.5, High, 2h]

### Workspace & Structure

- [x] Set up Rust workspace with crate structure (ftms-parser, ftms-parser-ffi, ble-transport) [1.1, High, 2h]
