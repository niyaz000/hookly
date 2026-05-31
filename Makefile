.PHONY: build run install migrate test fmt lint

build:
	cargo build

run:
	cargo run

install:
	cargo install sqlx-cli --no-default-features --features postgres

migrate:
	sqlx migrate run

test:
	cargo test

fmt:
	cargo fmt --all

lint:
	cargo fmt --all -- --check
	cargo clippy --all-targets -- -D warnings
