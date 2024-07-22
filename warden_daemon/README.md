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

    1. Launch Warden: sudo RUST_LOG=debug target/debug/warden_daemon -q "../realm/tools/qemu/build/qemu-system-aarch64" -u "/tmp/usocket1"
    2. Connect with socat: sudo socat - UNIX-CONNECT:/tmp/usocket1
    3. Run tcp server for comm with realm: nc -lvp 1338
    4. Use socat to create realm: {"CreateRealm":{"config":{"machine":"virt","cpu":{"cpu":"cortex-a57","cores_number":1},"memory":{"ram_size":4068},"network":{"vsock_cid":12346,"tap_device":"tap100","mac_address":"52:55:00:d1:55:01","hardware_device":"e1000","remote_terminal_uri":"tcp:localhost:1338"},"kernel":{"kernel_path":"{!!! PATH_TO_REALM_IMAGE !!!}"}}}}
    5. Use socat to start realm: {"StartRealm":{"uuid":"{!!! REALM_UUID !!!}"}}
    6. Use socat to stop realm: {"StopRealm":{"uuid":"{!!! REALM_UUID !!!}"}}