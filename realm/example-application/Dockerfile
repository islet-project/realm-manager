FROM rust:slim-bookworm AS build

WORKDIR /app

RUN --mount=type=bind,source=src,target=src \
    --mount=type=bind,source=Cargo.toml,target=Cargo.toml \
    --mount=type=bind,source=.cargo,target=.cargo \
    <<EOF
set -e
apt update
DEBIAN_FRONTEND=noninteractive apt install -y gcc-aarch64-linux-gnu
rustup target add aarch64-unknown-linux-gnu
cargo install --path . --root / --target=aarch64-unknown-linux-gnu
EOF

FROM scratch AS final

USER root:root

COPY --from=build /bin/exampleapp /bin/

ENTRYPOINT ["/bin/exampleapp"]
