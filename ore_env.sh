#!/bin/bash
#

if [ $# -ne 1 ]; then
	echo "USAGE: $0 [integer representing the miner number in ore_env.priv.sh]"
	exit 1
fi

source ./ore_env.priv.sh

# Determine the values of the miner from the parameter number passed and ore_env.priv.sh
RPCNO=RPC$1
KEYNO=KEY$1
THREADSNO=THREADS$1
FEENO=PRIORITY_FEE$1

RPC_URL=${!RPCNO}
KEY=${!KEYNO}
THREADS=${!THREADSNO}
FEE=${!FEENO}

# echo RPC_URL: ${RPC_URL}
# echo KEY: ${KEY}
# echo THREADS: ${THREADS}
# echo FEE: ${FEE}

# Check that all required parameters have been specified for the miner number passed
if [ -v ${RPC_URL} ]; then
	echo "ERROR: No RPC URL has been detected for miner $1. Please configure RPC$1 in your ore_env.priv.sh file."
	exit 2
fi
if [ -v ${KEY} ]; then
	echo "ERROR: No key has been detected for miner $1. Please configure KEY$1 in your ore_env.priv.sh file."
	exit 3
fi
if [ ! -f ${KEY} ]; then
	echo "ERROR: the keyfile could not be located: ${KEY}. Please configure KEY$1 in your ore_env.priv.sh file."
	exit 4
fi
if [ -v ${THREADS} ]; then
	echo "ERROR: No number of threads has been detected for miner $1. Please configure THREADS$1 in your ore_env.priv.sh file."
	exit 5
fi
if [ -v ${FEE} ]; then
	if [[ ! "${FEE}" =~ ^[0-9]+$ ]]; then
		echo "ERROR: No default priority fee has been specified for miner $1. Please configure PRIORITY_FEE$1 in your ore_env.priv.sh file."
		exit 5
	fi
fi

# check that the ore-cli binary is present
ORE_BIN=./target/release/ore
if [ ! -f ${ORE_BIN} ]; then
	echo "Sorry, the ore-cli file does not exist: ${ORE_BIN}"
	exit 2
fi


