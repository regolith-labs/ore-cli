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
  -e RPC=mainnet \
  -e BUFFER_TIME=5 \
  -e THREAD=4 \
  -v /local/path/to/id.json:/ore/.config/solana/id.json:ro \
  ghcr.io/regolith-labs/ore:latest
```

### Environment Variables

- `RPC`: Select the RPC URL to use. Options: `mainnet`, `devnet`, or a custom URL. Default is `devnet`.
- `BUFFER_TIME`: Set the buffer time (in seconds).
- `THREAD`: Set the number of threads to use.

### Wallet Mapping

Ensure that you map your local wallet file `id.json` to the path `/ore/.config/solana/id.json` in the container so that `ore-cli` can securely access your Solana wallet.

Example:

```sh
docker run \
  -v /home/$USER/.config/solana/id.json:/ore/.config/solana/id.json:ro \
  ghcr.io/regolith-labs/ore:latest
```