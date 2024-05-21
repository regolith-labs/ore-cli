#!/bin/bash
#
#devnet config

if [ $# -ne 1 ]; then
	echo "USAGE: $0 [integer representing the miner number in ore_env.priv.sh]"
	exit 1
fi
source ./ore_env.sh $1


solana config set --url ${RPC1} >/dev/null

while true; do
	echo ------------------------------------------------------------------------------------------------------------------------
	echo Initialising:		${MINER_NAME}
	echo ------------------------------------------------------------------------------------------------------------------------
	echo Wallet:			${KEY}
	echo RPC:				${RPC_URL}
	echo Priority fee:		${FEE}
	echo Threads:			${THREADS}
	# echo Wattage Idle:		${MINER_WATTAGE_IDLE}W
	# echo Wattage Busy:		${MINER_WATTAGE_BUSY}W
	# echo Electricity Cost:	\$${MINER_COST_PER_KILOWATT_HOUR} / kWHr
	echo ore-cli:			${ORE_BIN}

	# echo `date +'%Y-%m-%d %H:%M:%S'` "Initial SOL Price:	\$${SOL_PRICE}"
	# echo `date +'%Y-%m-%d %H:%M:%S'` "Initial ORE Price:	\$${ORE_PRICE}"
	echo ------------------------------------------------------------------------------------------------------------------------
	export MINER_NAME
	export MINER_WATTAGE_IDLE
	export MINER_WATTAGE_BUSY
	export MINER_COST_PER_KILOWATT_HOUR 
	# start the miner
	COMMAND="${ORE_BIN} mine --rpc ${RPC_URL} --keypair ${KEY} --priority-fee=${FEE} --threads ${THREADS} --buffer-time 2"
	# echo ${COMMAND}
	eval $COMMAND
	[ $? -eq 0 ] && break

	echo ------------------------------------------------------------------------------------------------------------------------
	echo `date +'%Y-%m-%d %H:%M:%S'` "Restarting miner process in 10 seconds..."
	sleep 10
done