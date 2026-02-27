.PHONY: build test clippy clean coverage

build:
	cargo build --workspace

test:
	cargo test --workspace

clippy:
	cargo clippy --workspace -- -D warnings

coverage:
	cargo tarpaulin -p ftms-parser --out Stdout

clean:
	cargo clean
