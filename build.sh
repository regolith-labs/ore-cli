#!/bin/bash
#
echo ========================================================
echo
echo
echo
echo
echo
echo
echo
echo
echo
echo
echo
echo
echo
echo
echo
echo
echo
echo
echo
echo
echo ========================================================
cargo build --release
if [ $? -eq 0 ]; then
	echo ========================================================
	echo Starting new miner...
	echo ========================================================
	./miner.sh
fi
