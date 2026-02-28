# ble-transport Engineering Decisions

## Trait abstractions over btleplug (traits.rs)

`BlePeripheral` and `BleAdapter` trait abstractions sit between the crate's logic and btleplug's concrete types. This enables mock-based testing without Bluetooth hardware.

The `BtleplugPeripheral` and `BtleplugAdapter` newtypes delegate all calls to btleplug. These are excluded from test coverage as pure hardware delegation (analogous to the "file system access code" exemption in CLAUDE.md).

## Notification handling extracted from tokio::select! (transport.rs)

The `handle_notification()` and `send_disconnected()` functions were extracted from the `tokio::select!` loop body. This was done because tarpaulin has known instrumentation issues with macro-generated code inside `select!`. Extracting to named functions allows direct unit testing and accurate coverage measurement.

## Multi-thread runtime for transport integration tests

Transport tests that spawn background tasks via `tokio::spawn` use `#[tokio::test(flavor = "multi_thread", worker_threads = 2)]`. This avoids subtle deadlocks between the `watch` channel's read/write locks and tokio's single-threaded scheduler, where the test task and background task compete for the same thread.

## FtmsConnection generic over BlePeripheral

`FtmsConnection<P: BlePeripheral>` is generic over the peripheral type. In production, `P = BtleplugPeripheral`. In tests, `P = TestPeripheral` (a hand-rolled mock). This gives full control over notification streams, write logs, and failure injection.

## TrainerCommand::Disconnect returns None from serialize()

The `Disconnect` variant does not correspond to an FTMS control point op code. It is handled at the transport layer (disconnect the peripheral and update state) rather than being serialized and written.

## Service discovery retry with disconnect/reconnect

`connect_and_setup()` retries the entire connect + discover sequence up to 3 times via a `connect_and_discover()` helper. On Linux/BlueZ, btleplug's `connect()` internally calls `bluez-async`'s `connect_with_timeout()`, which runs `await_service_discovery()` after the BLE link is established. A "Service discovery timed out" error therefore surfaces from `connect()`, not `discover_services()`. The error is a `bluez_async::BluetoothError::ServiceDiscoveryTimedOut` wrapped as `btleplug::Error::Other(Box<dyn Error>)`. The retry loop wraps both `connect()` and `discover_services()` so that either failure triggers a disconnect and retry. Tests use a `ConnectFailPeripheral` wrapper mock that fails `connect()` a configurable number of times to simulate the real-world error path.

## Scanner returns (DiscoveredDevice, Peripheral) tuples

`scan_for_ftms_devices()` returns metadata alongside the peripheral so consumers can present device info to the user before passing the peripheral to `connect_to_trainer()`. This avoids coupling scanner and connection modules.

## Heart rate zero filtering (ftms-parser)

The JetBlack Volt V2 (and potentially other trainers) reports `heart_rate_bpm=0` in Indoor Bike Data notifications when fully stopped. Since 0 bpm is physiologically impossible (the BLE HR Service spec 0x2A37 uses 0 to mean "not available"), the parser filters HR=0 to `None`. This is done in the parser so all consumers (UI, session recording, etc.) get the fix automatically.

## Command debouncing (debounce.rs)

`CommandDebouncer` enforces a minimum 1-second interval between control commands written to the trainer. Some trainers reject rapid command sequences. The debouncer uses a "keep latest" strategy: when a command arrives too soon, it overwrites any existing pending command. The pending command fires when the interval elapses via a `tokio::select!` branch.

Safety-critical writes (ERG death spiral overrides, ramp ticks) bypass the debouncer entirely but call `record_write()` to reset the timer. `Disconnect` and `Reset` also bypass debouncing. The `CommandDebouncer` uses deterministic time injection (`Instant` parameters) for testability, following the same pattern as `ErgSafetyMonitor` in the safety crate.

## Feature capability validation (transport.rs, connection.rs)

The Feature characteristic (0x2ACC) is read at connect time and the parsed `FitnessMachineFeature` is stored in `FtmsCharacteristics`. Before sending control commands, `validate_command_feature()` checks the trainer's `TargetSettingFeatures` flags. The validation is **fail-open**: if feature parsing failed (e.g., read error, malformed data), commands are sent unconditionally. This prevents a parsing edge case from blocking a functional trainer. Only when a feature is explicitly absent does the command get skipped with a warning log.
