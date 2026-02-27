.PHONY: build test clippy clean

build:
	cargo build --workspace

test:
	cargo test --workspace

clippy:
	cargo clippy --workspace -- -D warnings

clean:
	cargo clean
