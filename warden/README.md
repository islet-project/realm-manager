# Warden Daemon


Warden workspace, contains crates (more info about crates in their folders):

1. Warden daemon
2. Warden daemon's CMD client
3. Warden daemon's clients' lib

## Building on arm

    RUSTFLAGS='-C target-feature=+crt-static' cargo build --target=aarch64-unknown-linux-gnu --config target.aarch64-unknown-linux-gnu.linker=\"aarch64-linux-gnu-gcc\" -r


## Testing

### Running Unit Tests

    cargo test
