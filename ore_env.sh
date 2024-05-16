#!/bin/bash
#
source ./ore_env.priv.sh

ORE_BIN=~/ore2/ore-cli/target/release/ore
DEFAULT_RPC_URL=${RPC1}
DEFAULT_KEY=~/.config/solana/id.json
#DEFAULT_KEY=./id.ore_miner1.json
DEFAULT_FEE=1050011
DEFAULT_FEE=500011
DEFAULT_FEE=100011
DEFAULT_FEE=50011
DEFAULT_FEE=10011
DEFAULT_FEE=11

DEFAULT_THREADS=3	# Don't kill the PC

# Lookup the current SOL price
for i in 1 2 3 4 5
do
	SOL_PRICE=$(curl -s "https://api.coingecko.com/api/v3/simple/price?ids=solana&vs_currencies=usd&x_cg_demo_api_key=${COINGECKO_APIKEY}" | jq '.solana.usd')
	if [ "${SOL_PRICE}" != "null" ]; then
		break
	fi
	if [ "${SOL_PRICE}" == "null" ]; then
		echo "`date +'%Y-%m-%d %H:%M:%S'` Failed to download the current SOL price :("
		SOL_PRICE="0.00"
	fi
done
export SOL_PRICE

# Lookup the current ORE price
for i in 1 2 3 4 5
do
	ORE_PRICE=$(curl -s "https://api.coingecko.com/api/v3/simple/price?ids=ore&vs_currencies=usd&x_cg_demo_api_key=${COINGECKO_APIKEY}" | jq '.ore.usd')
	if [ "${ORE_PRICE}" != "null" ]; then
		break
	fi
	if [ "${ORE_PRICE}" == "null" ]; then
		echo "`date +'%Y-%m-%d %H:%M:%S'` Failed to download the current ORE price :("
		ORE_PRICE="0.00"
	fi
done
export ORE_PRICE
