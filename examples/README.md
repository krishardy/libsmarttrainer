# libsmarttrainer Examples

Runnable examples demonstrating the libsmarttrainer library API.

## Prerequisites

- A Bluetooth adapter (USB or built-in)
- An FTMS-compatible smart trainer, powered on and in range
- Linux: your user must be in the `bluetooth` group (`sudo usermod -aG bluetooth $USER`, then log out/in)

## Examples

| Example | Description | Run command |
|---|---|---|
| `scan` | Discover FTMS trainers over BLE | `cargo run --example scan` |
| `connect` | Connect to a trainer, hold until Ctrl+C | `cargo run --example connect` |
| `read_data` | Stream real-time speed/cadence/power/HR | `cargo run --example read_data` |
| `write_data` | Send ERG, resistance, and simulation commands | `cargo run --example write_data` |
| `full_workflow` | End-to-end: scan, connect, write, read, disconnect | `cargo run --example full_workflow` |

## Running

From the `libsmarttrainer/` directory:

```sh
# Run a specific example
cargo run --example scan

# Or use make targets
make run-example-scan
make run-example-connect
make run-example-read
make run-example-write
make run-example-full

# Compile all examples without running (no hardware needed)
make build-examples
```
