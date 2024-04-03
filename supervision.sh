#!/bin/bash

# Check if at least one argument is given
if [ $# -lt 1 ]; then
    echo "Usage: $0 <RPC_URL>"
    exit 1
fi

# Assign the first argument to RPC_URL
RPC_URL=$1

# Command and its arguments, with dynamic RPC URL
COMMAND="./target/release/ore --rpc ${RPC_URL} --keypair ~/.config/solana/id2.json --priority-fee 5000000 mine --threads 8"

# Loop indefinitely
while true; do
  echo "Starting the process..."
  
  # Execute the command
  eval $COMMAND
  
  # If the command was successful, exit the loop
  # Remove this if you always want to restart regardless of exit status
  [ $? -eq 0 ] && break
  
  echo "Process exited with an error. Restarting in 5 seconds..."
  sleep 5
done
