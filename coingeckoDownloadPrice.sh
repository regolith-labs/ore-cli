#!/bin/bash
#

source ./ore_env.priv.sh

if [ $# -ne 1 ]; then
	echo "USAGE: $0 [Ore|Sol]"
	exit 1
fi

if [[ ! "$1" == "Ore" && ! "$1" == "Sol" ]]; then
	echo "USAGE: $0 [Ore|Sol]"
	exit 1
fi

FILENAME=currentPriceOf${1}.txt

TOKENNAME="solana"
if [ "$1" = "Ore" ]; then
	TOKENNAME="ore"
fi

if [ -z ${COINGECKO_APIKEY} ]; then
	echo "ERROR: you do not appear to have a unique COINGECKO_APIKEY defined in your ore_env.priv.sh. Please obtain one to use the coingecko api to obtain a token price."
	echo "See https://docs.coingecko.com/reference/setting-up-your-api-key for further information"
	echo Failed to obtain current ORE price: writing \$0.00 to ${FILENAME}
	echo 0.00 > ${FILENAME}
	exit 2
fi

# Lookup the current price
for i in 1 2 3 4 5
do
	PRICE=$(curl -s "https://api.coingecko.com/api/v3/simple/price?ids=${TOKENNAME}&vs_currencies=usd&x_cg_demo_api_key=${COINGECKO_APIKEY}" | jq ".${TOKENNAME}.usd")
	if [ "${PRICE}" == "null" ]; then
		echo "`date +'%Y-%m-%d %H:%M:%S'` Failed to download the current $1 price :("
		PRICE="0.00"
	else
		break
	fi
done
echo Current $1 price: writing \$${PRICE} to ${FILENAME}
echo ${PRICE} > ${FILENAME}
