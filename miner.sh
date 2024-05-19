#!/bin/bash
#
#devnet config

if [ $# -ne 1 ]; then
	echo "USAGE: $0 [integer representing the miner number in ore_env.priv.sh]"
	exit 1
fi
source ./ore_env.sh $1

MINER_NAME="Miner ${1:-1}"

solana config set --url ${RPC1} >/dev/null

while true; do
	echo ----------------------------------------------------------------------------------------------------
	echo Starting		${MINER_NAME}
	echo ----------------------------------------------------------------------------------------------------
	echo Wallet:		${KEY}
	echo RPC:			${RPC_URL}
	echo Threads:		${THREADS}
	echo Priority fee:	${FEE}
	echo ore-cli:		${ORE_BIN}

	./coingeckoDownloadPrice.sh Ore
	./coingeckoDownloadPrice.sh Sol
	# echo `date +'%Y-%m-%d %H:%M:%S'` "Initial SOL Price:	\$${SOL_PRICE}"
	# echo `date +'%Y-%m-%d %H:%M:%S'` "Initial ORE Price:	\$${ORE_PRICE}"
	echo ----------------------------------------------------------------------------------------------------
	# start the miner
	COMMAND="${ORE_BIN} mine --rpc ${RPC_URL} --keypair ${KEY} --priority-fee=${FEE} --threads ${THREADS} --buffer-time 2"
	# echo ${COMMAND}
	eval $COMMAND
	[ $? -eq 0 ] && break
	# echo `date +'%Y-%m-%d %H:%M:%S'` "Restart in 5 seconds..."
	# sleep 5
done