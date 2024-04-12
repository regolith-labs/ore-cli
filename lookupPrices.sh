
INITIAL_SOL_PRICE=$(curl -s "https://api.coingecko.com/api/v3/simple/price?ids=solana&vs_currencies=usd" | jq '.solana.usd')
if [ "${INITIAL_SOL_PRICE}" == "null" ]; then
	echo "`date +'%Y-%m-%d %H:%M:%S'` Failed to download the current SOL price :("
	INITIAL_SOL_PRICE="0.00"
fi
export SOL_PRICE
INITIAL_ORE_PRICE=$(curl -s "https://api.coingecko.com/api/v3/simple/price?ids=ore&vs_currencies=usd" | jq '.ore.usd')
if [ "${INITIAL_ORE_PRICE}" == "null" ]; then
	echo "`date +'%Y-%m-%d %H:%M:%S'` Failed to download the current ORE price :("
	INITIAL_ORE_PRICE="0.00"
fi
export ORE_PRICE
