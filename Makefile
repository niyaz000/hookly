.PHONY: build run install migrate test

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
