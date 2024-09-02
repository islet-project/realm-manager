# Warden Daemon

Warden daemon that runs and manages realms and applications that are inside them.

## Building

    cargo build

## Running

### Command-line formula

    sudo RUST_LOG=debug target/debug/warden_daemon -q "../realm/tools/qemu/build/qemu-system-aarch64" -u "/tmp/usocket1" -w target/debug/warden_daemon_workdir -p 1337

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
First you need to compile Realm's kernel and tools: `../../realm/README.md`.
Then fill empty envs in the undermentioned command and run.

    cargo test --no-run
    sudo RUST_TEST_TIME_INTEGRATION=240 RUST_LOG=trace REALM_QEMU_PATH=... REALM_KERNEL_PATH=... !TEST_BINARY! --ignored --nocapture

Defaulted envs:

- WARDEN_VSOCK_PORT=1337
- TAP_DEVICE=tap100
- REALM_STARTUP_TIMEOUT=60
- NAT_NETWORK_NAME = virtbDaemonTest
- NAT_NETWORK_IP = 192.168.100.1/24

### E2E Tests

    Use warden command-line client: ../cmd_client