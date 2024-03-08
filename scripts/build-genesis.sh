#!/bin/bash

# Genesis head and code generator

echo "Build genesis head and code for local"

./target/release/mythical-node build-spec\
  --disable-default-bootnode\
  --chain=mainnet-dev > ./resources/mythical-shell-local.json

./target/release/mythical-node build-spec\
  --chain ./resources/mythical-shell-local.json\
  --raw\
  --disable-default-bootnode > ./resources/mythical-shell-local-raw.json

./target/release/mythical-node export-genesis-state --chain ./resources/mythical-shell-local-raw.json > ./resources/mythical-shell-local-head-data
./target/release/mythical-node export-genesis-wasm --chain ./resources/mythical-shell-local-raw.json > ./resources/mythical-shell-local-code