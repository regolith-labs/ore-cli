#!/bin/bash
#
source ./ore_env.sh
KEY=${1:-$DEFAULT_KEY}
while true; do
	# echo ----------------------------------------------------------------------------------------
	./unclaimedbalance.sh -k=${KEY}
	sleep 60
done