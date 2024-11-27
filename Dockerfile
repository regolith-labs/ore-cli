FROM rust:bookworm AS builder

RUN apt-get update && apt-get install -y \
    openssl \
    pkg-config \
    libssl-dev

WORKDIR /usr/src/ore-cli

COPY Cargo.toml Cargo.lock rust-toolchain.toml .
COPY src ./src

RUN cargo clean && cargo update && cargo build --release

FROM registry.access.redhat.com/ubi9/ubi-minimal AS intermediate

RUN microdnf update -y && \
    groupadd -g 1000 ore && \
    useradd -u 1000 -g ore -d /ore -m ore && \
    mkdir -p /ore/.config/solana && chown -R ore:ore /ore/.config

FROM registry.access.redhat.com/ubi9/ubi-micro

COPY --from=intermediate /usr/bin/grep /usr/bin/grep
COPY --from=intermediate /etc/passwd /etc/passwd
COPY --from=intermediate /etc/group /etc/group
COPY --from=intermediate /etc/shadow /etc/shadow
COPY --from=intermediate --chown=ore:ore --chmod=500 /ore /ore
COPY --from=intermediate /usr/lib64/libssl.so.3 /usr/lib64/libssl.so.3
COPY --from=intermediate /usr/lib64/libcrypto.so.3 /usr/lib64/libcrypto.so.3
COPY --from=intermediate /usr/lib64/libz.so.1 /usr/lib64/libz.so.1
COPY --from=intermediate /usr/lib64/libpcre.so.1 /usr/lib64/libpcre.so.1
COPY --from=intermediate /usr/lib64/libsigsegv.so.2 /usr/lib64/libsigsegv.so.2

WORKDIR /ore

COPY --from=builder --chown=ore:ore --chmod=500 /usr/src/ore-cli/target/release/ore /usr/local/bin/ore
COPY --chown=ore:ore --chmod=500 entrypoint.sh /usr/local/bin/entrypoint.sh

USER ore

ENTRYPOINT ["/usr/local/bin/entrypoint.sh"]