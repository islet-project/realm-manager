# Warden Daemon

Warden daemon that runs and manages realms and applications that are inside them.

## Building

    cargo build

## Running

### Command-line formula

    sudo RUST_LOG=debug target/debug/warden_daemon -q "../realm/tools/qemu/build/qemu-system-aarch64" -u "/tmp/usocket1"

### All possible cmd args

| Name | Description | Default value |
|-|-|-|
|--qemu-path | Path to qemu that runs realms | N/A|
|--warden-workdir-path | Path where Warden's dir is/will be located | N/A|
|--unix-sock-path | Path to socket on which Warden is listening for Clients' connections | N/A|
|--cid| CID on which Warden daemon listens | 2 (VMADDR_CID_HOST)|
|--port| Port on which Warden daemon listens | 80|
|--realm-connection-wait-time-secs | Timeout for realm's connection to Warden after start | 60 sec|

## Testing

### Running Unit Tests

    cargo test

### Running Integration Tests

    REALM_QEMU_PATH=... REALM_KERNEL_PATH=... cargo test  -- --ignored

### E2E Tests

    Use warden command-line client: ../cmd_client