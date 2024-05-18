#!/bin/bash
#
# This will only work when connected to devnet.

source ./ore_env.priv.sh
# Set local env to defaults
RPC_URL=$DEFAULT_RPC_URL
KEY=$DEFAULT_KEY
FEE=$DEFAULT_FEE

if [ $# -lt 1 ]; then
	echo USAGE: $0 [amount_of_sol] [optional:path/to/key.json]
	exit 1
fi
if [ $# -gt 2 ]; then
	echo USAGE: $0 [amount_of_sol] [optional:path/to/key.json]
	exit 2
fi

if [ $# -eq 2 ]; then
	KEY=$2
fi

if [ ! -f ${KEY} ]; then
	echo ERROR: could not find key file: ${KEY}
	exit 3
fi

echo Requesting an airdrop of $1 SOL...
echo ------------------------------------------------------------------
solana config set --url ${RPC1} >/dev/null
solana airdrop -v $1 ${KEY}
echo ------------------------------------------------------------------
echo Wallet now has $(solana  -k ${KEY} balance) available
