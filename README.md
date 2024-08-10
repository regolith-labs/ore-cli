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
choco install openssl pkgconfiglite
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

## Running with Docker

Run the Docker image with your wallet mapped:

```sh
docker run -it \
  -e RPC=mainnet \
  -e BUFFER_TIME=5 \
  -e CORES=4 \
  -v /local/path/to/id.json:/ore/id.json:ro \
  ghcr.io/regolith-labs/ore:latest
```

### Functions

The Docker image supports the following functions:

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
- `transfer`: Send ORE to anyone, anywhere in the world.
- `upgrade`: Upgrade your ORE tokens from v1 to v2.

### Environment Variables

- `RPC`: Set the RPC URL (mainnet, devnet, or custom URL). Default is `devnet`.
- `BUFFER_TIME`: The number of seconds before the deadline to stop mining and start submitting (default: 5).
- `CORES`: Number of CPU cores to allocate to mining (default: 1).
- `PRIORITY_FEE`: Price to pay for compute units. If dynamic fee URL is also set, this value will be the max (default: 500000).
- `DYNAMIC_FEE_URL`: RPC URL to use for dynamic fee estimation.
- `DYNAMIC_FEE_STRATEGY`: Strategy to use for dynamic fee estimation. Must be one of 'helius' or 'triton'.

### Volumes

To use your wallet files, mount them as volumes:

- Mount your wallet file:
    ```sh
    -v /path/to/your/id.json:/ore/id.json
    ```
- Mount your payer wallet file, which is used to pay fees for transactions:
    ```sh
    -v /path/to/your/payer.json:/ore/payer.json
    ```

### Examples

- Display balance account:
    ```sh
    docker run --rm -it \
      -v /path/to/your/id.json:/ore/id.json:ro \
      ghcr.io/regolith-labs/ore:latest balance
    ```
- Display help:
    ```sh
    docker run --rm -it ghcr.io/regolith-labs/ore:latest --help
    ```
- Benchmark your hashpower:
    ```sh
    docker run --rm -it ghcr.io/regolith-labs/ore:latest benchmark
    ```