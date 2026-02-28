# TODOs

## TODO

### BLE Transport (ble-transport crate)

- [ ] Build ble-transport crate: async scan/connect/subscribe with tokio::sync::watch channel API [2.1, High, 6h]
- [ ] Connection state tracking (connecting/connected/disconnected state machine) [2.8, Med, 1h]
- [ ] Automatic reconnection logic on BLE drop [2.9, Med, 3h]
- [ ] When in ERG mode, avoid death spiral by monitoring the cadence. If it drops below 40 rpm for 3 seconds, temporarily turn off ERG mode in the trainer until the cadence increases to 85 rpm. Then turn ERG mode back on and ramp the setpoint from 0 to the setpoint over 15 seconds. [2.9a]

### Hardening

- [ ] Graceful error types: BLE permission denied, trainer not found, characteristic missing [4.4, High, 2h]
- [ ] Multiple trainer support (handle >1 FTMS device in range) [4.5, Low, 2h]
- [ ] Trainer-specific quirk testing and workarounds [4.7, Med, 4h]

### Documention

- [ ] Create library usage examples
      - Scanning
      - Connecting
      - Reading data
      - Writing data

## Done


### FTMS Protocol (ftms-parser crate)

- [x] Implement Indoor Bike Data parser (0x2AD2): flags bitfield walking, speed/cadence/power extraction [1.2, High, 4h]
- [x] Implement Control Point command serializer (0x2AD9): request control, set target power, resistance, grade [1.3, High, 2h]
- [x] Implement Feature characteristic parser (0x2ACC) [1.4, Med, 1h]
- [x] Unit tests with hardcoded byte payloads for all parser functions [1.5, High, 2h]
- [x] Capture real notification payloads and add as test vectors [1.8, Med, 1h]

### Workspace & Structure

- [x] Set up Rust workspace with crate structure (ftms-parser, ble-transport) [1.1, High, 2h]
