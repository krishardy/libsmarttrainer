.PHONY: build test test-ftms test-ble test-safety test-quirks clippy clean coverage coverage-ble \
       build-examples run-example-scan run-example-connect run-example-read run-example-write run-example-full

build:
	cargo build --workspace

test:
	cargo test --workspace

test-ftms:
	cargo test -p ftms-parser

test-ble:
	cargo test -p ble-transport

test-safety:
	cargo test -p safety

test-quirks:
	cargo test -p trainer-quirks

clippy:
	cargo clippy --workspace -- -D warnings

coverage:
	cargo tarpaulin -p ftms-parser -p ble-transport -p safety -p trainer-quirks --out Stdout

coverage-ble:
	cargo tarpaulin -p ble-transport --exclude-files "ble-transport/src/traits.rs" --exclude-files "ftms-parser/src/lib.rs" --out Stdout

build-examples:
	cargo build -p ble-transport --examples

run-example-scan:
	cargo run -p ble-transport --example scan

run-example-connect:
	cargo run -p ble-transport --example connect

run-example-read:
	cargo run -p ble-transport --example read_data

run-example-write:
	cargo run -p ble-transport --example write_data

run-example-full:
	cargo run -p ble-transport --example full_workflow

clean:
	cargo clean
