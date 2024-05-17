#!/bin/bash
#
source ./ore_env.priv.sh

solana config set --url ${RPC1}
# solana config set --url localhost
if [ "$1" = "new" ]; then
	solana-keygen new --force
fi
solana-keygen verify $(solana-keygen pubkey) ~/.config/solana/id.json
solana airdrop $2