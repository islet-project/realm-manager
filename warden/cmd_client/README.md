# Warden command-line client

This crate provide a simple cli interface to the warden daemon.

## Building

    cargo build

## Running

### Command-line formula

    RUST_LOG=trace cargo run -- -u "/tmp/usocket123"

### Example commands:
|Command| Usage |
|-|-|
|Create realm| create-realm -k {ABSOLUTE_PATH_TO_BUILT_KERNEL} -v {VSOCK_CID_FOR_REALM} [-u {TCP_SERVER_URI}]|
|Start realm| start-realm -r {REALM_ID}|
|Stop realm| stop-realm -r {REALM_ID}|
|Inspect realm| inspect-realm -r {REALM_ID}|
|Reboot realm| reboot-realm -r {REALM_ID}|
|Destroy realm| destroy-realm -r {REALM_ID}|
|List realms| list-realms|
|Create application| create-application -r {REALM_ID} -n {NAME} -v {VERSION} -i {IMAGE} -o {IMAGE_PARTITION_SIZE} -d {DATA_PARTITION_SIZE}|
|Update application| update-application -r {REALM_ID} -a {APPLICATION_ID} -n {NAME} -v {VERSION} -i {IMAGE} -o {IMAGE_PARTITION_SIZE} -d {DATA_PARTITION_SIZE}|
|Start application| start-application -r {REALM_ID} -a {APPLICATION_ID}|
|Stop application| stop-application -r {REALM_ID} -a {APPLICATION_ID}|
    
