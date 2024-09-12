# Warden Daemon

Warden daemon that runs and manages realms and applications that are inside them.

## Building

    cargo build

## Running
To successfully run the daemon you have to install:

    sudo apt-get install dnsmasq


### Example command-line formula

    sudo RUST_LOG=info target/debug/warden_daemon -q "../../realm/tools/qemu/build/qemu-system-aarch64" -u "/tmp/usocket123" -w "target/random12" -p 1337 -d "/usr/sbin/dnsmasq"

### All possible cmd args

| Name | Description | Default value |
|-|-|-|
|--qemu-path | Path to qemu that runs realms | N/A|
|--warden-workdir-path | Path where Warden's dir is/will be located | N/A|
|--unix-sock-path | Path to socket on which Warden is listening for Clients' connections | N/A|
|--dhcp-exec-path | Path to dhcp exec | N/A|
|--cid| CID on which Warden daemon listens | 2 (VMADDR_CID_HOST)|
|--port| Port on which Warden daemon listens | 80|
|--realm-connection-wait-time-secs | Timeout for realm's connection to Warden after start | 60 sec|
|--realm-response-wait-time-secs | Timeout for realm's response to Warden command | 10 sec|
|--bridge-name| Name of daemon's virtual interface | virtbWarden|
|--network-address| IP of daemon's virtual interface | 192.168.100.0/24|
|--dhcp-connections-number| Number of dhcp connections that is used to calculate dhcp range for server| 20|
|--dns-records| Additional records for Dnsmasq. Use following pattern: */\<domain\>\[/\<domain\>...\]/\[\<ipaddr\>\]* | N/A|


## Testing

### Running Unit Tests

    cargo test

### Running Integration Tests
First you need to compile Realm's kernel and tools: `../../realm/README.md`.
Then fill empty envs in the undermentioned command and run.

    cargo test --no-run
    sudo TAP_DEVICE=TAPTESTWARDEN RUST_TEST_TIME_INTEGRATION=240 RUST_LOG=trace NAT_NETWORK_NAME=... REALM_QEMU_PATH=... DHCP_EXEC_PATH=... REALM_KERNEL_PATH=... !TEST_BINARY! --ignored --nocapture

Defaulted envs:

- WARDEN_VSOCK_PORT=1337
- TAP_DEVICE=tap100
- REALM_STARTUP_TIMEOUT=60
- NAT_NETWORK_IP = 192.168.100.0/24

### E2E Tests

    Use warden command-line client: ../cmd_client