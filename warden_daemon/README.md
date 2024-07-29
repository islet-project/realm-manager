# Warden Daemon

## Building

    cargo build

## Running

### Command-line formula

    sudo RUST_LOG=debug target/debug/warden_daemon -q "../realm/tools/qemu/build/qemu-system-aarch64" -u "/tmp/usocket1"

## Testing

### Running Unit Tests

    cargo test

### E2E Tests

    Use warden command-line client: ../warden_cmd_client