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
buildexitcode=$?
if [ $# -ne 1 ]; then
	exit $buildexitcode
fi

if [ ! -f ./ore ]; then
	echo Creating a link to the ore executable
	ln -s ./target/release/ore ./ore
fi

if [ $buildexitcode -eq 0 ]; then
	echo ========================================================
	echo Starting miner $1...
	echo ========================================================
	./miner.sh $1
fi
