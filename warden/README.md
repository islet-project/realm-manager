# Warden Daemon


Warden workspace, contains crates (more info about crates in their folders):

1. Warden daemon
2. Warden daemon's CMD client
3. Warden daemon's clients' lib

## Building on arm

    make -C ../realm deps
    rustup target add aarch64-unknown-linux-gnu
    make


## Testing

### Running Unit Tests

    cargo test
