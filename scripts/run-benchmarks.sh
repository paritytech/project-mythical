#!/bin/bash

set -e

if [ "$#" -ne 1 ]; then
    echo "Usage: $0 [testnet|mainnet]"
    exit 1
fi

RUNTIME=$1

case $RUNTIME in
    testnet)
        ;;
    mainnet)
        ;;
    *)
        echo "Invalid parameter. Please use 'testnet' or 'mainnet'."
        exit 1
        ;;
esac

echo "Building the binary. This can take a while..."
cargo build --release --features runtime-benchmarks

BIN="./target/release/mythos-node"
BENCHMARKS=($($BIN benchmark pallet --list=pallets --no-csv-header --chain="$RUNTIME"))
WEIGHT_FOLDER="./runtime/$RUNTIME/src/weights"

# Benchmark the pallets
for PALLET in "${BENCHMARKS[@]}"; do
    echo "Generating benchmarks for $PALLET..."

    OUTPUT="$WEIGHT_FOLDER/$PALLET.rs"
    $BIN benchmark pallet \
        --chain "$RUNTIME" \
        --pallet "$PALLET" \
        --extrinsic "*" \
        --wasm-execution compiled \
        --steps 50 \
        --repeat 20 \
        --template ./.maintain/template.hbs \
        --output "$OUTPUT"
    echo "Benchmarks for $PALLET successfully generated in $OUTPUT"
done

# Benchmark overheads
$BIN benchmark overhead \
  --chain=dev \
  --wasm-execution=compiled \
  --weight-path "$WEIGHT_FOLDER" \
  --warmup=10 \
  --repeat=100
