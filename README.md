# Libsmarttrainer

Rust library for controlling BLE smart trainers via the FTMS protocol.

## Features

It is able to read:
* Power
* Cadence
* Speed
* Heart Rate (if paired with trainer)

It is able to write:
* ERG (target power)
* Resistance (target level)
* Indoor Bike Simulation (grade, rolling resistance, wind resistance)

## Modules

- **`parser`** — FTMS protocol parser (Indoor Bike Data, Feature flags, Control Point responses/commands)
- **`quirks`** — Device-specific post-processing (e.g., HR=0 filtering for JetBlack Volt V2)
- **`safety`** — ERG death spiral protection (cadence monitoring, power suspension/ramp)
- **`ble`** *(feature-gated)* — BLE scanning, connection management, and FTMS control via btleplug

## Feature Flags

| Feature | Default | Description |
|---|---|---|
| `ble` | Yes | Full BLE transport layer (btleplug, tokio, async runtime) |

Without the `ble` feature, you get lightweight parser/quirks/safety modules with no async or BLE dependencies.

## Usage

```toml
[dependencies]
# Full BLE support (default)
libsmarttrainer = "0.1"

# Parser/quirks/safety only (no BLE dependencies)
libsmarttrainer = { version = "0.1", default-features = false }
```

## Examples

See [`examples/`](examples/) for runnable examples demonstrating scanning, connecting, reading data, and writing commands.

```sh
# Compile all examples (no hardware needed)
make build-examples

# Run a specific example (requires BLE adapter and trainer)
make run-example-scan
```
