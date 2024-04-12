#bin/bash
#
# This script will start up a basic ore-cli miner

# Enter the URL for your desired RPC server
DEFAULT_RPC_URL="https://quaint-proud-dew.solana-mainnet.quiknode.pro/XXXXXXXXXXXXXXXXXXXXXX/"
# Enter the path to your local wallet config file
DEFAULT_KEY=~/.config/solana/id.ore_web.json
MINER_NAME="Miner 1"

DEFAULT_FEE=100
DEFAULT_THREADS=4

RPC_URL=$DEFAULT_RPC_URL
KEY=$DEFAULT_KEY
FEE=$DEFAULT_FEE
THREADS=$DEFAULT_THREADS

ORE_BIN=ore
# Redirect to use the compiled version of your client. Comment out if you are not compiling your own ore-cli
ORE_BIN=~/ore-cli/target/release/ore


while true; do
	echo -------------------------------------------------------------------------------
	echo `date +'%Y-%m-%d %H:%M:%S'` Starting ${MINER_NAME}.....
	echo `date +'%Y-%m-%d %H:%M:%S'` RPC: ${RPC_URL}
	echo `date +'%Y-%m-%d %H:%M:%S'` ore-cli: ${ORE_BIN}

	source ./lookupPrices.sh
	echo `date +'%Y-%m-%d %H:%M:%S'` "Initial SOL Price:	\$${SOL_PRICE}"
	echo `date +'%Y-%m-%d %H:%M:%S'` "Initial ORE Price:	\$${ORE_PRICE}"
	echo -------------------------------------------------------------------------------
	# start the miner
	COMMAND="${ORE_BIN} --rpc ${RPC_URL} --keypair ${KEY} --priority-fee ${FEE} --initial-sol-price ${SOL_PRICE} --initial-ore-price ${ORE_PRICE} mine --threads ${THREADS}"
	# echo ${COMMAND}
	eval $COMMAND
	[ $? -eq 0 ] && break
	echo `date +'%Y-%m-%d %H:%M:%S'` "Restart in 5 seconds..."
	sleep 5
done