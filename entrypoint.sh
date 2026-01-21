#!/bin/sh

MAINNET_URL="https://api.mainnet-beta.solana.com"
DEVNET_URL="https://api.devnet.solana.com"
WALLET_PATH="/ore/id.json"
WALLET_PAYER_PATH="/ore/payer.json"
FUNCTION="mine"

RED='\033[0;31m'
NC='\033[0m'

show_help() {
    echo "
Usage: $0 [OPTIONS] [FUNCTION]

Options:
  --help               Show this help message and exit

Functions:
  balance              Fetch an account balance
  benchmark            Benchmark your hashpower
  busses               Fetch the bus account balances
  claim                Claim your mining rewards
  close                Close your account to recover rent
  config               Fetch the program config
  mine                 Start mining
  proof                Fetch a proof account by address
  rewards              Fetch the current reward rate for each difficulty level
  stake                Stake to earn a rewards multiplier
  transfer             Send ORE to anyone, anywhere in the world.
  upgrade              Upgrade your ORE tokens from v1 to v2

Environment Variables:
  RPC                  Set the RPC URL (mainnet, devnet, or custom URL)
  BUFFER_TIME          The number seconds before the deadline to stop mining and start submitting [default: 5]
  CORES                The number of CPU cores to allocate to mining [default: 1]
  PRIORITY_FEE         Price to pay for compute unit. If dynamic fee url is also set, this value will be the max. [default: 500000]
  DYNAMIC_FEE_URL      RPC URL to use for dynamic fee estimation.
  DYNAMIC_FEE_STRATEGY Strategy to use for dynamic fee estimation. Must be one of 'helius', or 'triton'.
  JITO                 Add jito tip to the miner. [default: false]

Volumes:
  To use your wallet files, mount them as volumes:
  -v /path/to/your/id.json:/ore/id.json
      Mount your wallet file, allowing the ore binary to access your wallet.
  -v /path/to/your/payer.json:/ore/payer.json
      Mount your payer wallet file, which is used to pay fees for transactions if the mine function is executed.

Examples:
  docker run -it -v /path/to/your/id.json:/ore/id.json myimage --help
  docker run -it -v /path/to/your/id.json:/ore/id.json myimage mine
  docker run -it -v /path/to/your/id.json:/ore/id.json myimage benchmark
  docker run -it -v /path/to/your/id.json:/ore/id.json myimage config
  docker run -it -v /path/to/your/id.json:/ore/id.json myimage rewards
  docker run -it -v /path/to/your/id.json:/ore/id.json myimage busses
"
}

set_rpc_url() {
    case "$RPC" in
        mainnet) RPC_URL="$MAINNET_URL" ;;
        devnet | "") RPC_URL="$DEVNET_URL" ;;
        *) RPC_URL="$RPC" ;;
    esac
}

display_header() {
    echo "
#############################################
#           _____  _____  _____             #
#          |     || __  ||   __|            #
#          |  |  ||    -||   __|            #
#          |_____||__|__||_____|            #
#                                           #
#############################################
#   RPC URL: ${RPC_URL}
#   BUFFER TIME: ${BUFFER_TIME:-5}
#   CORES COUNT: ${CORES:-1}
#   PRIORITY FEE: ${PRIORITY_FEE:-500000}
#   DYNAMIC FEE URL: ${DYNAMIC_FEE_URL:-Not set}
#   DYNAMIC FEE STRATEGY: ${DYNAMIC_FEE_STRATEGY:-Not set}
"
}

validate_params() {
    if [ "$FUNCTION" != "benchmark" ] && [ "$FUNCTION" != "config" ] && [ "$FUNCTION" != "rewards" ] && [ "$FUNCTION" != "busses" ]; then
        [ ! -f "$WALLET_PATH" ] && echo -e "${RED}Error: Wallet file not found at $WALLET_PATH${NC}" && exit 1
    fi
    [ ! -x "$(command -v ore)" ] && echo -e "${RED}Error: ore binary not found or not executable${NC}" && exit 1
    [ -z "$RPC_URL" ] && RPC_URL="$DEVNET_URL"
    ! echo "$RPC_URL" | grep -qE '^https?://' && echo -e "${RED}Error: Invalid RPC_URL: $RPC_URL${NC}" && exit 1
    [ -n "$BUFFER_TIME" ] && ! echo "$BUFFER_TIME" | grep -qE '^[0-9]+$' && echo -e "${RED}Error: BUFFER_TIME must be a positive integer${NC}" && exit 1
    [ -n "$CORES" ] && ! echo "$CORES" | grep -qE '^[1-9][0-9]*$' && echo -e "${RED}Error: CORES must be a positive integer${NC}" && exit 1
    [ -n "$PRIORITY_FEE" ] && ! echo "$PRIORITY_FEE" | grep -qE '^[0-9]+$' && echo -e "${RED}Error: PRIORITY_FEE must be a non-negative integer${NC}" && exit 1
    [ -n "$DYNAMIC_FEE_URL" ] && ! echo "$DYNAMIC_FEE_URL" | grep -qE '^https?://' && echo -e "${RED}Error: Invalid DYNAMIC_FEE_URL: $DYNAMIC_FEE_URL${NC}" && exit 1
    [ -n "$DYNAMIC_FEE_STRATEGY" ] && ! echo "$DYNAMIC_FEE_STRATEGY" | grep -qE '^(helius|triton)$' && echo -e "${RED}Error: DYNAMIC_FEE_STRATEGY must be 'helius' or 'triton'${NC}" && exit 1
    [ -n "$JITO" ] && ! echo "$JITO" | grep -qE '^(true|false)$' && echo -e "${RED}Error: JITO must be 'true' or 'false'${NC}" && exit 1
}

build_command() {
    if [ "$FUNCTION" = "benchmark" ] || [ "$FUNCTION" = "config" ] || [ "$FUNCTION" = "rewards" ] || [ "$FUNCTION" = "busses" ]; then
        cmd="ore $FUNCTION"
        [ -n "$CORES" ] && cmd="$cmd --cores \"$CORES\""
    else
        cmd="ore --keypair \"$WALLET_PATH\" --rpc \"$RPC_URL\" $FUNCTION"
        [ -f "$WALLET_PAYER_PATH" ] && cmd="$cmd --fee-payer \"$WALLET_PAYER_PATH\""
        if [ "$FUNCTION" = "mine" ]; then
            [ -n "$BUFFER_TIME" ] && cmd="$cmd --buffer-time \"$BUFFER_TIME\""
            [ -n "$CORES" ] && cmd="$cmd --cores \"$CORES\""
            [ -n "$PRIORITY_FEE" ] && cmd="$cmd --priority-fee \"$PRIORITY_FEE\""
            [ -n "$DYNAMIC_FEE_URL" ] && cmd="$cmd --dynamic-fee-url \"$DYNAMIC_FEE_URL\""
            [ -n "$DYNAMIC_FEE_STRATEGY" ] && cmd="$cmd --dynamic-fee-strategy \"$DYNAMIC_FEE_STRATEGY\""
            [ "$JITO" = "true" ] && cmd="$cmd --jito"
        fi
    fi
}

execute_command() {
    sh -c "$cmd"
    if [ $? -ne 0 ]; then
        echo -e "${RED}Error: ore binary exited with non-zero status${NC}"
        echo "Command executed: $cmd"
        exit 1
    fi
}

for arg in "$@"
do
    case "$arg" in
        --help) show_help; exit 0 ;;
        balance|benchmark|busses|claim|close|config|proof|rewards|stake|upgrade) FUNCTION="$arg" ;;
    esac
done

set_rpc_url
if [ "$FUNCTION" = "mine" ]; then
    display_header
fi
validate_params
build_command
execute_command