#!/bin/bash

set -e

ZOMBIENET_V=v1.3.128
POLKADOT_V=polkadot-stable2412-4
POLKADOT_RUNTIMES_V=v1.4.2
PASEO_RUNTIMES_V=v1.4.1

case "$(uname -s)" in
Linux*) MACHINE=Linux ;;
Darwin*) MACHINE=Mac ;;
*) exit 1 ;;
esac

if [ $MACHINE = "Linux" ]; then
  ZOMBIENET_FILE="zombienet-linux-x64"
  IS_LINUX=1
elif [ $MACHINE = "Mac" ]; then
  ZOMBIENET_FILE="zombienet-macos"
  IS_LINUX=0
fi

SCRIPT_DIR="$( cd -- "$(dirname "$0")" >/dev/null 2>&1 ; pwd -P )"
BIN_DIR="$SCRIPT_DIR/bin"
TEMP_FOLDER="$SCRIPT_DIR/tmp"
ZOMBIENET_BIN="${BIN_DIR}/zombienet"
mkdir -p "$BIN_DIR"

build_polkadot() {
  echo "cloning polkadot repository..."
  pushd /tmp
  git clone --depth 1 --branch "$POLKADOT_V" https://github.com/paritytech/polkadot-sdk.git || echo -n
  pushd polkadot-sdk
  echo "building polkadot executable..."
  cargo build --release --features fast-runtime
  cp target/release/polkadot "$BIN_DIR"
  cp target/release/polkadot-execute-worker "$BIN_DIR"
  cp target/release/polkadot-prepare-worker "$BIN_DIR"
  cargo build --release -p polkadot-parachain-bin
  cp target/release/polkadot-parachain "$BIN_DIR"
  popd
  popd
}

build_chainspec_generators() {
  echo "cloning chain-spec-generators..."
  pushd /tmp
  if [ ! -f "$BIN_DIR/chain-spec-generator" ]; then
    git clone --depth 1 --branch "$POLKADOT_RUNTIMES_V" https://github.com/polkadot-fellows/runtimes.git polkadot-runtimes || echo -n
    pushd polkadot-runtimes
    echo "building polkadot chain-spec-generator..."
    cargo build --release --features fast-runtime
    cp target/release/chain-spec-generator "$BIN_DIR/chain-spec-generator"
    popd
  fi
  if [ ! -f "$BIN_DIR/paseo-chain-spec-generator" ]; then
    git clone --depth 1 --branch "$PASEO_RUNTIMES_V" https://github.com/paseo-network/runtimes.git paseo-runtimes || echo -n
    pushd paseo-runtimes
    echo "building paseo chain-spec-generator..."
    cargo build --release --features fast-runtime
    cp target/release/chain-spec-generator "$BIN_DIR/paseo-chain-spec-generator"
    popd
  fi
  popd
}

fetch_polkadot() {
  echo "fetching from polkadot repository..."
  echo "$BIN_DIR"
  pushd "$BIN_DIR"
  wget https://github.com/paritytech/polkadot-sdk/releases/download/polkadot-$POLKADOT_V/polkadot
  wget https://github.com/paritytech/polkadot-sdk/releases/download/polkadot-$POLKADOT_V/polkadot-execute-worker
  wget https://github.com/paritytech/polkadot-sdk/releases/download/polkadot-$POLKADOT_V/polkadot-prepare-worker
  chmod +x ./*
  popd
}

zombienet_init() {
  if [ ! -f "$ZOMBIENET_BIN" ]; then
    echo "fetching zombienet executable..."
    curl -o "$ZOMBIENET_BIN" -LO "https://github.com/paritytech/zombienet/releases/download/$ZOMBIENET_V/$ZOMBIENET_FILE-arm64"
    chmod +x "$ZOMBIENET_BIN"
  fi
  build_chainspec_generators
  if [ ! -f "$BIN_DIR/polkadot" ]; then
    if [ "$IS_LINUX" -eq 1 ]; then
      fetch_polkadot
    else
      build_polkadot
    fi
  fi
}

zombienet_testnet() {
  zombienet_init
  cargo build --release --features testnet-runtime/metadata-hash
  echo "spawning paseo-local relay chain plus mythos testnet as a parachain..."
  rm -rf "$TEMP_FOLDER"
  $ZOMBIENET_BIN spawn zombienet-config/testnet.toml -p native -d "$TEMP_FOLDER"
}

zombienet_testnet_asset_hub() {
  zombienet_init
  cargo build --release --features testnet-runtime/metadata-hash
  echo "spawning paseo-local relay chain plus muse testnet as a parachain plus asset-hub..."
  rm -rf "$TEMP_FOLDER"
  $ZOMBIENET_BIN spawn zombienet-config/testnet-asset-hub.toml -p native -d "$TEMP_FOLDER"
}

zombienet_mainnet() {
  zombienet_init
  cargo build --release --features mainnet-runtime/metadata-hash
  echo "spawning paseo-local relay chain plus mythos mainnet as a parachain..."
  rm -rf "$TEMP_FOLDER"
  $ZOMBIENET_BIN spawn zombienet-config/mainnet.toml -p native -d "$TEMP_FOLDER"
}

zombienet_mainnet_asset_hub() {
  zombienet_init
  cargo build --release --features mainnet-runtime/metadata-hash
  echo "spawning polkadot-local relay chain plus mythos mainnet as a parachain plus asset-hub..."
  rm -rf "$TEMP_FOLDER"
  $ZOMBIENET_BIN spawn zombienet-config/mainnet-asset-hub.toml -p native -d "$TEMP_FOLDER"
}

print_help() {
  echo "This is a shell script to automate the execution of zombienet."
  echo ""
  echo "$ ./zombienet.sh init                       # fetches zombienet and polkadot executables"
  echo "$ ./zombienet.sh build                      # builds polkadot executables from source"
  echo "$ ./zombienet.sh testnet                    # spawns a paseo-local relay chain plus muse testnet-local as a parachain"
  echo "$ ./zombienet.sh testnet_asset_hub          # spawns a paseo-local relay chain plus muse testnet-local as a parachain plus asset-hub"
  echo "$ ./zombienet.sh mainnet                    # spawns a polkadot-local relay chain plus mythos mainnet-local as a parachain"
  echo "$ ./zombienet.sh mainnet_asset_hub          # spawns a polkadot-local relay chain plus mythos mainnet-local as a parachain plus asset-hub"
}

SUBCOMMAND=$1
case $SUBCOMMAND in
"" | "-h" | "--help")
  print_help
  ;;
*)
  shift
  zombienet_"$SUBCOMMAND" $@
  if [ $? = 127 ]; then
    echo "Error: '$SUBCOMMAND' is not a known SUBCOMMAND." >&2
    echo "Run './zombienet.sh --help' for a list of known subcommands." >&2
    exit 1
  fi
  ;;
esac
