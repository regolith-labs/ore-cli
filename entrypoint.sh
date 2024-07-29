#!/bin/sh
##############################################
#                                            #
#   ORE Mining Launcher                      #
#                                            #
#   Configure RPC URL and launch ORE         #
#   Proof of Work mining on Solana network   #
#   with customizable parameters.            #
#                                            #
#   Author: KlementXV                        #
#   Date: 2024-07-29                         #
#   Version: 1.0                             #
#                                            #
##############################################

MAINNET_URL="https://api.mainnet-beta.solana.com"
DEVNET_URL="https://api.devnet.solana.com"
WALLET_PATH="/ore/.config/solana/id.json"
VERSION=$(grep '^version' Cargo.toml | awk -F' = ' '{print $2}' | tr -d '"')

display_header() {
    echo "
##############################################
           _____  _____  _____
          |     || __  ||   __|
          |  |  ||    -||   __|
          |_____||__|__||_____|

##############################################
VERSION: $VERSION
RPC_URL: ${RPC_URL:-Not set, using DEVNET}
BUFFER_TIME: ${BUFFER_TIME:-Not set}
THREAD_COUNT: ${THREAD_COUNT:-Not set}
##############################################
"
}

check_wallet_file() {
    if [ ! -f "$WALLET_PATH" ]; then
        echo "Error: Wallet file not found at $WALLET_PATH"
        echo "Please ensure the wallet file is present before running the miner."
        exit 1
    fi
}

check_ore_binary() {
    if [ ! -x "$(command -v ore)" ]; then
        echo "Error: ore binary not found or not executable"
        exit 1
    fi
}

check_rpc_url() {
    if ! echo "$RPC_URL" | grep -qE '^https?://'; then
        echo "Error: Invalid RPC_URL: $RPC_URL"
        exit 1
    fi
}

check_buffer_time() {
    if [ -n "$BUFFER_TIME" ] && ! [[ "$BUFFER_TIME" =~ ^[0-9]+$ ]]; then
        echo "Error: BUFFER_TIME must be a positive integer"
        exit 1
    fi
}

check_thread_count() {
    if [ -n "$THREAD_COUNT" ] && ! [[ "$THREAD_COUNT" =~ ^[1-9][0-9]*$ ]]; then
        echo "Error: THREAD_COUNT must be a positive integer"
        exit 1
    fi
}

set_rpc_url() {
    case "$RPC_URL" in
      mainnet)
        RPC_URL="$MAINNET_URL"
        ;;
      devnet|""|unset)
        RPC_URL="$DEVNET_URL"
        ;;
      *)
        ;;
    esac
}

build_command() {
    cmd="./ore --rpc \"$RPC_URL\" mine"
    [ -n "$BUFFER_TIME" ] && cmd="$cmd --buffer-time \"$BUFFER_TIME\""
    [ -n "$THREAD_COUNT" ] && cmd="$cmd --threads \"$THREAD_COUNT\""
}

execute_command() {
    sh -c "$cmd"
    if [ $? -ne 0 ]; then
        echo "Error: ore binary exited with non-zero status"
        exit 1
    fi
}

display_header
check_wallet_file
check_ore_binary
check_rpc_url
check_buffer_time
check_thread_count
set_rpc_url
build_command
execute_command
