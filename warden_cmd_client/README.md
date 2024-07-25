# Warden command-line client

## Building

    cargo build

## Running

### Command-line formula

    sudo RUST_LOG=debug target/debug/warden_cmd_client -u "/tmp/usocket1"

### Example commands:

    - Create realm: create-realm -k {ABSOLUTE_PATH_TO_BUILT_KERNEL} -v {VSOCK_CID_FOR_REALM} [-u {TCP_SERVER_URI}]
    - Start realm: start-realm -r {REALM_ID}
    - Stop realm: stop-realm -r {REALM_ID}
    - Inspect realm: inspect-realm -r {REALM_ID}
    - Reboot realm: reboot-realm -r {REALM_ID}
    - Destroy realm: destroy-realm -r {REALM_ID}
    - List realms: list-realms
    - Create application: create-application -r {REALM_ID}
    - Update application: update-application -r {REALM_ID} -a {APPLICATION_ID}
    - Start application: start-application -r {REALM_ID} -a {APPLICATION_ID}
    - Stop application: stop-application -r {REALM_ID} -a {APPLICATION_ID}
    