#!/bin/bash

source ./ore_env.sh

# Set local env to defaults
RPC_URL=$DEFAULT_RPC_URL
KEY=$DEFAULT_KEY
FEE=$DEFAULT_FEE

# Override env from command line
for i in "$@"; do
  case $i in
    -r=*|--rpc=*)
      RPC_URL="${i#*=}"
      shift # past argument=value
      ;;
    -k=*|--key=*)
      KEY="${i#*=}"
      shift # past argument=value
      ;;
    -f=*|--fee=*)
      FEE="${i#*=}"
      shift # past argument=value
      ;;
    # --default)
    #   DEFAULT=YES
    #   shift # past argument with no value
    #   ;;
    -*|--*)
      echo "Unknown option $i"
	  echo "Usage: $0 --key=key_file --rpc=RPC_URL --fee=12345"
      echo "Usage: $0 -k=key_file -r=RPC_URL -f=12345"
      exit 1
      ;;
    *)
      ;;
  esac
done
echo ------------------------------------------------------------------------------------------
cutoff=0.10
# unclaimed=$(${ORE_BIN} --rpc=${RPC1} --keypair ${KEY} rewards)
# u=$(echo "${unclaimed}" | tr -dc '0-9.')
# valueString=$(echo "${u} ${ORE_PRICE}" | awk '{print $1 * $2}')
balance=$(${ORE_BIN} --rpc ${RPC_URL} --keypair ${KEY} balance)
retval1=$?
balanceVal=$(echo ${balance} | awk '{printf("%.10f", $2)}')
stakedVal=$(echo ${balance} | awk '{printf("%.10f", $5)}')

./unclaimedbalance.sh
echo "Wallet has ${balanceVal} ready to add to current stake ${stakedVal}"
echo ------------------------------------------------------------------------------------------
echo "Staking additional ${stakedVal} ORE...."
balance=$(${ORE_BIN} --rpc ${RPC_URL} --keypair ${KEY} --priority-fee=${FEE} stake)
echo ------------------------------------------------------------------------------------------
./unclaimedbalance.sh
