default:
    @just --list

build:
    cargo build --release --bin cron-person
    mkdir -p bin
    cp target/release/cron-person bin/cron-person

test:
    cargo test --all-targets

lint:
    cargo clippy --all-targets -- -D warnings

fmt:
    cargo fmt --package cron-person-cli

fmt-check:
    cargo fmt --package cron-person-cli -- --check

coverage:
    mkdir -p .tmp
    cargo llvm-cov --all-targets --fail-under-lines 90 --json --output-path .tmp/coverage-final.json

check: fmt-check lint test coverage

run *args:
    cargo run --bin cron-person -- {{args}}

run-ai *args:
    AGENT=1 cargo run --bin cron-person -- {{args}}
