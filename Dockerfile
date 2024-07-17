FROM rust:alpine AS builder

RUN apk add --no-cache musl-dev

WORKDIR /usr/src/ore-cli

COPY . .

RUN cargo build --release

FROM alpine:latest

RUN addgroup -S ore && adduser -S -G ore -h /ore ore && \
    apk update && apk upgrade libcrypto3 libssl3 && apk add --no-cache libgcc libstdc++ && \
    mkdir -p /ore/.config/solana && chown -R ore:ore /ore/.config

WORKDIR /usr/local/bin

COPY --from=builder --chown=ore:ore --chmod=500 /usr/src/ore-cli/target/release/ore /usr/local/bin/ore
COPY --chown=ore:ore --chmod=100 entrypoint.sh /usr/local/bin/entrypoint.sh

USER ore

ENTRYPOINT ["/usr/local/bin/entrypoint.sh"]