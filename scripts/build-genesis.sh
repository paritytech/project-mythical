#!/bin/bash

# Genesis head and code generator

echo "Build genesis head and code for local"

./target/release/mythos-node build-spec\
  --disable-default-bootnode\
  --chain=mainnet-dev > ./resources/mythos-shell-local.json

./target/release/mythos-node build-spec\
  --chain ./resources/mythos-shell-local.json\
  --raw\
  --disable-default-bootnode > ./resources/mythos-shell-local-raw.json

./target/release/mythos-node export-genesis-state --chain ./resources/mythos-shell-local-raw.json > ./resources/mythos-shell-local-head-data
./target/release/mythos-node export-genesis-wasm --chain ./resources/mythos-shell-local-raw.json > ./resources/mythos-shell-local-code
