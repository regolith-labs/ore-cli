# ORE CLI

A command line interface for ORE cryptocurrency mining.

## Install

To install the CLI, use [cargo](https://doc.rust-lang.org/cargo/getting-started/installation.html):

```sh
cargo install ore-cli
```

## Build

To build the codebase from scratch, checkout the repo and use cargo to build:

```sh
cargo build --release
```

## Help

You can use the `-h` flag on any command to pull up a help menu with documentation:

```sh
ore -h
```

## Running the Docker Image

To run the Docker image with your wallet mapped in read-only mode, use the following command:

```sh
docker run \
  -e RPC_URL=mainnet \
  -e BUFFER_TIME=5 \
  -e THREAD_COUNT=4 \
  -v /local/path/to/id.json:/ore/.config/solana/id.json:ro \
  ghcr.io/klementxv/ore:latest
```

### Environment Variables

- `RPC_URL`: Select the RPC URL to use. Options: `mainnet`, `devnet`, `testnet`, or a custom URL. Default is `devnet`.
- `BUFFER_TIME`: Set the buffer time (in seconds).
- `THREAD_COUNT`: Set the number of threads to use.

### Wallet Mapping

Ensure that you map your local wallet file `id.json` to the path `/ore/.config/solana/id.json` in the container in read-only mode (RO) so that `ore-cli` can securely access your Solana wallet.

Example:

```sh
docker run \
  -v /home/user/.config/solana/id.json:/ore/.config/solana/id.json:ro \
  ghcr.io/klementxv/ore:latest
```