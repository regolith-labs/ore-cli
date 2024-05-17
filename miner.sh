#!/bin/bash
#
#devnet config
source ./ore_env.sh

RPC_URL=$DEFAULT_RPC_URL
KEY=$DEFAULT_KEY
FEE=$DEFAULT_FEE
THREADS=$DEFAULT_THREADS

MINER_NAME="Miner ${1:-1}"

if [ ! -f ${KEY} ]; then
	echo "Sorry, the key file does not exist: ${KEY}"
	exit 2
fi

if [ ! -f ${ORE_BIN} ]; then
	echo "Sorry, the ore-cli file does not exist: ${ORE_BIN}"
	exit 2
fi

solana config set --url ${RPC1} >/dev/null

# while true; do
	echo ----------------------------------------------------------------------------------------------------
	echo Starting	${MINER_NAME}
	echo Wallet:	${KEY}
	echo RPC:		${RPC_URL}
	echo ore-cli:	${ORE_BIN}

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
# done