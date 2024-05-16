#!/bin/bash
#
solana config set --url devnet
#solana config set --url localhost
# solana-keygen new
solana-keygen verify $(solana-keygen pubkey) ~/.config/solana/id.json
solana airdrop 5