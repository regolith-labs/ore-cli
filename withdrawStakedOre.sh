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
balanceVal=$(echo ${balance} | awk '{printf("%.11f", $2)}')
stakedVal=$(echo ${balance} | awk '{printf("%.11f", $5)}')

./unclaimedbalance.sh
echo "This wallet can currently withdraw ${stakedVal} staked ORE"


valueString=1000
if [ "${stakedVal}" = "0.0000000000" ]; then
	valueString=0;
fi

if [ $(echo "${valueString} > ${cutoff}" | bc -l) -eq 1 ]; then
	echo ------------------------------------------------------------------------------------------
	echo `date +'%Y-%m-%d %H:%M:%S'` Your rewards of \$${valueString} are greater than \$${cutoff} so proceeding to claim rewards.
	echo `date +'%Y-%m-%d %H:%M:%S'` Wallet:	${KEY}
	echo `date +'%Y-%m-%d %H:%M:%S'` RPC:	${RPC_URL}
	echo `date +'%Y-%m-%d %H:%M:%S'` Priority Fee:	${FEE}
	echo `date +'%Y-%m-%d %H:%M:%S'` ore-cli:	${ORE_BIN}
	${ORE_BIN} --keypair ${KEY} --rpc ${RPC_URL} --priority-fee ${FEE} claim
	echo ------------------------------------------------------------------------------------------
	./unclaimedbalance.sh
else
	echo ------------------------------------------------------------------------------------------
	echo "Sorry, there in not enough staked ORE to justify spending SOL to withdraw it."
	echo "Please try again when you have more than \$${cutoff} waiting to be withdrawn."
	echo ------------------------------------------------------------------------------------------
fi
