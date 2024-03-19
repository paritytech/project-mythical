#!/bin/bash

set -e

chain=$1

# Check if chain is either "mythos" or "muse"
if [ "$chain" != "mythos" ] && [ "$chain" != "muse" ]; then
    echo "Error: Chain must be either 'mythos' or 'muse'"
    echo "Usage: $0 <chain>"
    exit 1
fi

echo "Build genesis head and code for chain $chain"

mkdir -p ./resources
./target/release/mythos-node build-spec --chain="$chain" > "./resources/$chain.json"

./target/release/mythos-node build-spec \
  --chain "./resources/$chain.json"     \
  --raw  > "./resources/$chain-raw.json"

./target/release/mythos-node export-genesis-state --chain "./resources/$chain-raw.json" > "./resources/$chain-head-data"
./target/release/mythos-node export-genesis-wasm --chain "./resources/$chain-raw.json" > "./resources/$chain-code"
