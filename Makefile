.PHONY: build test clippy clean coverage doc publish-dry-run \
       build-examples run-example-scan run-example-connect run-example-read run-example-write run-example-full

build:
	cargo build

test:
	cargo test

clippy:
	cargo clippy -- -D warnings

coverage:
	cargo tarpaulin --exclude-files "src/ble/traits.rs" --out Stdout

doc:
	cargo doc --no-deps

publish-dry-run:
	cargo publish --dry-run

build-examples:
	cargo build --examples

run-example-scan:
	cargo run --example scan

run-example-connect:
	cargo run --example connect

run-example-read:
	cargo run --example read_data

run-example-write:
	cargo run --example write_data

run-example-full:
	cargo run --example full_workflow

clean:
	cargo clean
