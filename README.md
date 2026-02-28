# Libsmarttrainer

Rust library that manages connections and interactions with a BLE smart trainer

It is able to read:
* Power
* Cadence
* Speed
* Heart Rate (if paired with trainer)

It is able to write:
* ERG (target power)
* Resistance (target level)
* Indoor Bike Simulation (grade, rolling resistance, wind resistance)

## Examples

See [`ble-transport/examples/`](ble-transport/examples/) for runnable examples demonstrating scanning, connecting, reading data, and writing commands.

```sh
# Compile all examples (no hardware needed)
make build-examples

# Run a specific example (requires BLE adapter and trainer)
make run-example-scan
```
