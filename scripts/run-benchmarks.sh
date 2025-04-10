#!/bin/bash

set -e

if [ "$#" -lt 1 ] || [ "$#" -gt 2 ]; then
    echo "Usage: $0 [testnet|mainnet] [optional: pallet1,pallet2,...]"
    exit 1
fi

RUNTIME=$1
PALLETS_ARG=$2

case $RUNTIME in
    testnet)
        CHAIN=local-v
        ;;
    mainnet)
        CHAIN=mainnet-local-v
        ;;
    *)
        echo "Invalid parameter. Please use 'testnet' or 'mainnet'."
        exit 1
        ;;
esac

echo "Building the binary. This can take a while..."
cargo build --profile production --features runtime-benchmarks

BIN="./target/production/mythos-node"
WEIGHT_FOLDER="./runtime/$RUNTIME/src/weights"

# Determine which pallets to benchmark
if [ -n "$PALLETS_ARG" ]; then
    IFS=',' read -r -a BENCHMARKS <<< "$PALLETS_ARG"
else
    BENCHMARKS=($($BIN benchmark pallet --list=pallets --no-csv-header --chain="$CHAIN"))
fi

# Benchmark the pallets
for PALLET in "${BENCHMARKS[@]}"; do
    echo "Generating benchmarks for $PALLET..."

    OUTPUT="$WEIGHT_FOLDER/$PALLET.rs"
    $BIN benchmark pallet \
        --chain "$CHAIN" \
        --pallet "$PALLET" \
        --extrinsic "*" \
        --wasm-execution compiled \
        --steps 50 \
        --repeat 20 \
        --output "$OUTPUT"
    echo "Benchmarks for $PALLET successfully generated in $OUTPUT"
done
