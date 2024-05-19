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
if [ "$1" = "nomine" ]; then
	exit $buildexitcode
fi

if [ ! -f ./ore ]; then
	echo Creating a link to the ore executable
	ln -s ./target/release/ore ./ore
fi

if [ $buildexitcode -eq 0 ]; then
	echo ========================================================
	echo Starting new miner...
	echo ========================================================
	./miner.sh
fi
