# ORE CLI

A command line interface for ORE cryptocurrency mining.

## Install

To install the CLI, use [cargo](https://doc.rust-lang.org/cargo/getting-started/installation.html):

```sh
cargo install ore-cli
```


### Dependencies
If you run into issues during installation, please install the following dependencies for your operating system and try again:

#### Linux
```
sudo apt-get install openssl pkg-config libssl-dev
```

#### MacOS (using [Homebrew](https://brew.sh/))
```
brew install openssl pkg-config

# If you encounter issues with OpenSSL, you might need to set the following environment variables:
export PATH="/usr/local/opt/openssl/bin:$PATH"
export LDFLAGS="-L/usr/local/opt/openssl/lib"
export CPPFLAGS="-I/usr/local/opt/openssl/include"
```

#### Windows (using [Chocolatey](https://chocolatey.org/))
```
choco install pkgconfiglite
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
  -v /local/path/to/id.json:/ore/id.json:ro \
  ghcr.io/regolith-labs/ore:latest
```

### Environment Variables

- `RPC`: Select the RPC URL to use. Options: `mainnet`, `devnet`, or a custom URL. Default is `devnet`.
- `BUFFER_TIME`: Set the buffer time.
- `THREAD`: Set the number of threads to use.

### Wallet Mapping

Ensure that you map your local wallet files `id.json` and `payer.json` (if needed) to the paths `/ore/id.json` and `/ore/payer.json` in the container.

Example:

```sh
docker run \
  -v /home/$USER/.config/solana/id.json:/ore/id.json:ro \
  -v /home/$USER/.config/solana/payer.json:/ore/payer.json:ro \
  ghcr.io/regolith-labs/ore:latest
```

### Functions

The ORE CLI supports various functions. You can specify the function to execute as an argument to the Docker run command. Here are the available functions:

- `balance`: Fetch an account balance.
- `benchmark`: Benchmark your hashpower.
- `busses`: Fetch the bus account balances.
- `claim`: Claim your mining rewards.
- `close`: Close your account to recover rent.
- `config`: Fetch the program config.
- `mine`: Start mining.
- `proof`: Fetch a proof account by address.
- `rewards`: Fetch the current reward rate for each difficulty level.
- `stake`: Stake to earn a rewards multiplier.
- `upgrade`: Upgrade your ORE tokens from v1 to v2.

### Examples

To start mining with the Docker image:

```sh
docker run \
  -v /path/to/your/id.json:/ore/id.json:ro \
  -e RPC=mainnet \
  -e BUFFER_TIME=5 \
  -e THREAD=4 \
  ghcr.io/regolith-labs/ore:latest mine
```

To benchmark your hashpower:

```sh
docker run -e THREAD=4 ghcr.io/regolith-labs/ore:latest benchmark
```

To fetch an account balance:

```sh
docker run \
  -v /path/to/your/id.json:/ore/id.json:ro \
  ghcr.io/regolith-labs/ore:latest balance
```

For detailed help on each function, use the `--help` flag:

```sh
docker run -it ghcr.io/regolith-labs/ore:latest --help
```