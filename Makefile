.PHONY: build test check fmt fmt-check lint clippy run doctor clean ci

build:
	cargo build

test:
	cargo test

fmt:
	cargo fmt

fmt-check:
	cargo fmt --check

lint clippy:
	cargo clippy --all-targets -- -D warnings

check: fmt-check lint test

run:
	cargo run --

doctor:
	cargo run -q -- doctor

clean:
	cargo clean

ci: build check
