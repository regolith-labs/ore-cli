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
	echo Buffer Time:		${BUFFER_TIME}
	# echo Wattage Idle:		${MINER_WATTAGE_IDLE}W
	# echo Wattage Busy:		${MINER_WATTAGE_BUSY}W
	# echo Electricity Cost:	\$${MINER_COST_PER_KILOWATT_HOUR} / kWHr
	echo ore-cli:			${ORE_BIN}

	WALLET_NAME=${KEY##*/}
	WALLET_NAME=${WALLET_NAME%.*}
	echo ------------------------------------------------------------------------------------------------------------------------
	export MINER_NAME
	export WALLET_NAME
	export MINER_WATTAGE_IDLE
	export MINER_WATTAGE_BUSY
	export MINER_COST_PER_KILOWATT_HOUR 
	export MINER_DESIRED_DIFFICULTY_LEVEL 

	if [ ! -d "./logs" ]; then
		mkdir "./logs"
	fi
	STATS_LOGFILE_BASE="./logs/${MINER_NAME// /_}"
	
	# rotate any previous logs to keep last 6
	ls ${STATS_LOGFILE_BASE}--6--*.log >/dev/null 2>1
	if [ $# -eq 0 ]; then
		for oldlog in $(ls ${STATS_LOGFILE_BASE}--6--*.log); do
			if [ -f "${oldlog}" ]; then
				rm "${oldlog}"
			fi
		done
	fi
	rotateLog() {
		origIndex=$1
		newIndex=$2
		ls ${STATS_LOGFILE_BASE}--${origIndex}--*.log >/dev/null 2>1
		if [ $# -eq 0 ]; then
			for oldlog in $(ls ${STATS_LOGFILE_BASE}--${origIndex}--*.log); do
				if [ -f "${oldlog}" ]; then
					newlog="${oldlog/--${origIndex}--/--${newIndex}--}"
					echo Rotating log: ${oldlog} -> ${newlog}
					mv ${oldlog} ${newlog}
					if [ ${origIndex} -eq 1 ]; then
						echo -e "*** This is an archived log file ***\n\n$(cat ${newlog})" > ${newlog}
					fi
				fi
			done
		fi
	}
	rotateLog 5 6
	rotateLog 4 5
	rotateLog 3 4
	rotateLog 2 3
	rotateLog 1 2
	STATS_LOGFILE="${STATS_LOGFILE_BASE}--1--$(date '+%Y-%m-%d-%H%M%S').log"
	# echo $LOGFILE
	export STATS_LOGFILE

	# start the miner
	COMMAND="${ORE_BIN} mine --rpc ${RPC_URL} --keypair ${KEY} --priority-fee=${FEE:-0} --threads ${THREADS:-1} --buffer-time ${BUFFER_TIME:-2}"
	# echo ${COMMAND}
	eval $COMMAND
	[ $? -eq 0 ] && break

	echo ------------------------------------------------------------------------------------------------------------------------
	echo `date +'%Y-%m-%d %H:%M:%S'` "Restarting miner process in 10 seconds..."
	sleep 10
done