#!/bin/bash
#
echo ------------------------------------------------------------
echo `date +'%Y-%m-%d %H:%M:%S'` Building release version of ore-cli...
cargo build --release; 
echo ------------------------------------------------------------
echo "`date +'%Y-%m-%d %H:%M:%S'` Built version: $(target/release/ore --version)"
