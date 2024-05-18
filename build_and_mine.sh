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

echo Creating a link
ln -s ./target/release/ore ./ore

if [ $buildexitcode -eq 0 ]; then
	echo ========================================================
	echo Starting new miner...
	echo ========================================================
	./miner.sh
fi
