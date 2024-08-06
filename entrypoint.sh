#!/bin/sh

MAINNET_URL="https://api.mainnet-beta.solana.com"
DEVNET_URL="https://api.devnet.solana.com"
WALLET_PATH="/ore/.config/solana/id.json"

RED='\033[0;31m'
NC='\033[0m'

set_rpc_url() {
    case "$RPC" in
        mainnet) RPC_URL="$MAINNET_URL" ;;
        devnet | "") RPC_URL="$DEVNET_URL" ;;
        *) RPC_URL="$RPC" ;;
    esac
}

display_header() {
    echo "
##############################################
           _____  _____  _____
          |     || __  ||   __|
          |  |  ||    -||   __|
          |_____||__|__||_____|

##############################################
RPC URL: ${RPC_URL}
BUFFER TIME: ${BUFFER_TIME:-Not set}
THREAD COUNT: ${THREAD:-Not set}
PRIORITY FEE: ${PRIORITY_FEE:-0}
##############################################
"
}

validate_params() {
    [ ! -f "$WALLET_PATH" ] && echo -e "${RED}Error: Wallet file not found at $WALLET_PATH${NC}" && exit 1
    [ ! -x "$(command -v ore)" ] && echo -e "${RED}Error: ore binary not found or not executable${NC}" && exit 1
    [ -z "$RPC_URL" ] && RPC_URL="$DEVNET_URL"
    ! echo "$RPC_URL" | grep -qE '^https?://' && echo -e "${RED}Error: Invalid RPC_URL: $RPC_URL${NC}" && exit 1
    [ -n "$BUFFER_TIME" ] && ! [[ "$BUFFER_TIME" =~ ^[0-9]+$ ]] && echo -e "${RED}Error: BUFFER_TIME must be a positive integer${NC}" && exit 1
    [ -n "$THREAD" ] && ! [[ "$THREAD" =~ ^[1-9][0-9]*$ ]] && echo -e "${RED}Error: THREAD must be a positive integer${NC}" && exit 1
    [ -n "$PRIORITY_FEE" ] && ! [[ "$PRIORITY_FEE" =~ ^[0-9]+$ ]] && echo -e "${RED}Error: PRIORITY_FEE must be a non-negative integer${NC}" && exit 1
}

build_command() {
    cmd="./ore --rpc \"$RPC_URL\" mine"
    [ -n "$BUFFER_TIME" ] && cmd="$cmd --buffer-time \"$BUFFER_TIME\""
    [ -n "$THREAD" ] && cmd="$cmd --threads \"$THREAD\""
    [ -n "$PRIORITY_FEE" ] && cmd="$cmd --priority-fee \"$PRIORITY_FEE\""
}

execute_command() {
    sh -c "$cmd"
    if [ $? -ne 0 ]; then
        echo -e "${RED}Error: ore binary exited with non-zero status${NC}"
        echo "Command executed: $cmd"
        exit 1
    fi
}

set_rpc_url
display_header
validate_params
build_command
execute_command