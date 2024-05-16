#!/bin/bash
#
source ./ore_env.sh
# Set local env to defaults
RPC_URL=$DEFAULT_RPC_URL
KEY=$DEFAULT_KEY
FEE=$DEFAULT_FEE

if [ ! -f ${KEY} ]; then
	echo "Sorry, the key file does not exist: ${KEY}"
	exit 2
fi

if [ ! -f ${ORE_BIN} ]; then
	echo "Sorry, the ore-cli file does not exist: ${ORE_BIN}"
	exit 2
fi

# Override env from command line
for i in "$@"; do
  case $i in
    -k=*|--key=*)
      KEY="${i#*=}"
      shift # past argument=value
      ;;
    -*|--*)
      echo "Unknown option $i"
	  echo "Usage: $0 --key=key_file"
      echo "Usage: $0 -k=key_file"
      exit 1
      ;;
    *)
      ;;
  esac
done
# echo `date +'%Y-%m-%d %H:%M:%S'` Wallet:	${KEY}
# echo `date +'%Y-%m-%d %H:%M:%S'` ore-cli:	${ORE_BIN}
SHORT_KEY=$(basename ${KEY})

echo -n "`date +'%Y%m%d%H%M%S'` ${SHORT_KEY} "
balance=$(${ORE_BIN} --rpc ${RPC_URL} --keypair ${KEY} balance)
retval1=$?
# rewards=$(${ORE_BIN} --rpc ${RPC_URL} --keypair ${KEY} rewards)
# retval2=$?

# Display Balance: XXX Staked: XXXX
echo "$(echo ${balance} | awk '{printf("%s %.11f %s %.11f", $1,$2,$4,$5)}')"

# u=$(echo "${rewards}" | tr -dc '0-9.')
# b=$(echo "${balance}" | tr -dc '0-9.')
# sum=$(echo ${b} ${u} | awk '{print $1 + $2}')
# value=$(echo ${sum} ${ORE_PRICE} | awk '{print $1 * $2}')
# value=$(printf "%'.2f" ${value})

#echo "${b} + ${u}(U) = ${sum} (\$${value} @ \$${ORE_PRICE})"

if [ ${retval1} -ne 0 ]; then
	echo "ERROR: Failed to retrieve the balance"
	exit ${retval1}
fi
# if [ ${retval2} -ne 0 ]; then
# 	echo "ERROR: Failed to retrieve the balance"
# 	exit ${retval2}
# fi

if [ "${unclaimed}" = "0.000000000 ORE" ]; then
	echo "There are no unclaimed rewards waiting."
# else
	# echo "Treasure waiting..."
fi
