# Warden Daemon

Warden daemon that runs and manages realms and applications that are inside them.

## Building

    cargo build

## Running

To successfully run the daemon you have to install udhcpd:

    sudo apt-get install udhcpd
    touch /var/lib/misc/udhcpd.leases


Then edit it's config :

    sudo vim /etc/udhcpd.conf

with following entries:

    start           VIRT_IF_DHCP_POOL_BEGIN
    end             VIRT_IF_DHCP_POOL_END
    interface       VIRT_IF_NAME
    option  subnet  VIRT_IF_MAKS
    opt     router  VIRT_IF_IP + 1


### Command-line formula

    sudo RUST_LOG=debug target/debug/warden_daemon -q "../realm/tools/qemu/build/qemu-system-aarch64" -u "/tmp/usocket1" -w target/debug/warden_daemon_workdir -p 1337 -d "/usr/sbin/udhcpd"

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
|--bridge_name| Name of daemon's virtual interface | virtbWarden|
|--bridge_ip| IP of daemon's virtual interface | 192.168.100.0/24|

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