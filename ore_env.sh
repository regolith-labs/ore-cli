#!/bin/bash
#
source ./ore_env.priv.sh

ORE_BIN=./target/release/ore
DEFAULT_RPC_URL=${RPC1}
DEFAULT_KEY=${KEY1}
DEFAULT_FEE=${PRIORITY_FEE_0}
DEFAULT_THREADS=3	# Don't kill the PC

# Lookup the current SOL price
# for i in 1 2 3 4 5
# do
# 	SOL_PRICE=$(curl -s "https://api.coingecko.com/api/v3/simple/price?ids=solana&vs_currencies=usd&x_cg_demo_api_key=${COINGECKO_APIKEY}" | jq '.solana.usd')
# 	if [ "${SOL_PRICE}" != "null" ]; then
# 		break
# 	fi
# 	if [ "${SOL_PRICE}" == "null" ]; then
# 		echo "`date +'%Y-%m-%d %H:%M:%S'` Failed to download the current SOL price :("
# 		SOL_PRICE="0.00"
# 	fi
# done
# export SOL_PRICE

# # Lookup the current ORE price
# for i in 1 2 3 4 5
# do
# 	ORE_PRICE=$(curl -s "https://api.coingecko.com/api/v3/simple/price?ids=ore&vs_currencies=usd&x_cg_demo_api_key=${COINGECKO_APIKEY}" | jq '.ore.usd')
# 	if [ "${ORE_PRICE}" != "null" ]; then
# 		break
# 	fi
# 	if [ "${ORE_PRICE}" == "null" ]; then
# 		echo "`date +'%Y-%m-%d %H:%M:%S'` Failed to download the current ORE price :("
# 		ORE_PRICE="0.00"
# 	fi
# done
# export ORE_PRICE
