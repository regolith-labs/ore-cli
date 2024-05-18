#!/bin/bash
#
source ./ore_env.priv.sh

if [ $# -ne 1 ]; then
	echo USAGE: $0 [/path/to/new/wallet.json]
	echo The default wallet is normally something like ~/.config/solana/id.json
	exit 1
fi

if [ -f $1 ]; then
	echo ERROR: a wallet file already exists at $1
	exit 1
fi

echo Attempting to create new solana wallet...$1
echo ------------------------------------------------------------------
solana config set --url ${RPC1} >/dev/null
solana-keygen new --outfile $1
solana-keygen verify $(solana-keygen pubkey $1) $1
echo ------------------------------------------------------------------
echo Wallet has $(solana -k $1 balance) available
echo If you are connected to devnet, you can get airdropped some Sol:
echo ./airdropDevnetSol.sh 1 $1
echo
echo ------------------------------------------------------------------
echo Write down and store the seed phrase above if you want to import and use this wallet somewhere else e.g. Phantom Wallet
echo You can also paste the contents of $1 into a wallet as your private key.
echo $1 contains your private key to the new solana wallet and should not be distributed to anyone if you value what is in the wallet.
echo $(solana-keygen pubkey $1) is your wallet address for sending any tokens to.
echo ------------------------------------------------------------------

