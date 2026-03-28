default:
    @just --list

# Run the app in development mode
dev:
    cargo run -- --port 1337

# Build the release binary
build:
    cargo build --release

# Run the release binary
run:
    cargo run --release -- --port 1337

# Run clippy lints
lint:
    cargo clippy --all-targets

# Format code
format:
    cargo fmt

# Check formatting
format-check:
    cargo fmt -- --check

# Run all checks (lint + format)
check: lint format-check

# Install the binary to ~/.cargo/bin
install:
    cargo install --path .

# Install and register as a macOS launch agent (runs on login)
install-service: install
    #!/usr/bin/env bash
    set -euo pipefail
    BINARY="$(which portmap)"
    PLIST=~/Library/LaunchAgents/dev.portmap.plist
    # Stop existing service if running
    launchctl bootout gui/$(id -u) "$PLIST" 2>/dev/null || true
    # Generate plist with correct binary path
    sed "s|BINARY_PATH|$BINARY|g" dev.portmap.plist > "$PLIST"
    launchctl bootstrap gui/$(id -u) "$PLIST"
    echo "Installed and started. Dashboard at http://localhost:1337"

# Stop and remove the macOS launch agent
uninstall-service:
    #!/usr/bin/env bash
    set -euo pipefail
    PLIST=~/Library/LaunchAgents/dev.portmap.plist
    launchctl bootout gui/$(id -u) "$PLIST" 2>/dev/null || true
    rm -f "$PLIST"
    echo "Service removed."

# Show service status
status:
    launchctl print gui/$(id -u)/dev.portmap 2>/dev/null || echo "Not running"

# View service logs
logs:
    tail -f /tmp/portmap.log
