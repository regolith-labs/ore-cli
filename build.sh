#!/bin/bash
#
cargo build --release
if [ $? -eq 0 ]; then
	./miner.sh
fi
