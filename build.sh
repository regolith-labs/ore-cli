#!/bin/bash
#
echo ------------------------------------------------
echo Building release version of ore-cli...
cargo build --release; 
echo ------------------------------------------------
echo "Built version: $(target/release/ore --version)"
