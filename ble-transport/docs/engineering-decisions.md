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

`connect_and_setup()` retries `discover_services()` up to 3 times. On failure, it disconnects and reconnects before retrying because BlueZ leaves the BLE connection in an indeterminate state after a discovery timeout. This mirrors the existing `request_control_with_retry` pattern. Tests use a `DiscoveryFailPeripheral` wrapper mock that delegates to `TestPeripheral` while injecting a configurable number of `TimedOut` errors.

## Scanner returns (DiscoveredDevice, Peripheral) tuples

`scan_for_ftms_devices()` returns metadata alongside the peripheral so consumers can present device info to the user before passing the peripheral to `connect_to_trainer()`. This avoids coupling scanner and connection modules.
