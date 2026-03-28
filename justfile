default:
    @just --list

# Run in development mode
dev:
    cargo run -- serve

# Build release binary
build:
    cargo build --release

# Run clippy lints
lint:
    cargo clippy --all-targets

# Format code
format:
    cargo fmt

# Check formatting
format-check:
    cargo fmt -- --check

# Run tests
test:
    cargo test

# Run all checks (lint + format + test)
check: lint format-check test

# Install binary to ~/.cargo/bin
install:
    cargo install --path .
