.PHONY: build test test-ftms clippy clean coverage

build:
	cargo build --workspace

test:
	cargo test --workspace

test-ftms:
	cargo test -p ftms-parser

clippy:
	cargo clippy --workspace -- -D warnings

coverage:
	cargo tarpaulin -p ftms-parser --out Stdout

clean:
	cargo clean
