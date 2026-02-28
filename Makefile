.PHONY: build test test-ftms test-ble test-safety clippy clean coverage coverage-ble

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

clippy:
	cargo clippy --workspace -- -D warnings

coverage:
	cargo tarpaulin -p ftms-parser -p ble-transport -p safety --out Stdout

coverage-ble:
	cargo tarpaulin -p ble-transport --exclude-files "ble-transport/src/traits.rs" --exclude-files "ftms-parser/src/lib.rs" --out Stdout

clean:
	cargo clean
