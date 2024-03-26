#!/bin/bash

set -e

ZOMBIENET_V=v1.3.95
POLKADOT_V=v1.8.0

case "$(uname -s)" in
    Linux*)     MACHINE=Linux;;
    Darwin*)    MACHINE=Mac;;
    *)          exit 1
esac

if [ $MACHINE = "Linux" ]; then
  ZOMBIENET_BIN=zombienet-linux-x64
  IS_LINUX=1
elif [ $MACHINE = "Mac" ]; then
  ZOMBIENET_BIN=zombienet-macos
  IS_LINUX=0
fi

BIN_DIR=bin

build_polkadot() {
  echo "cloning polkadot repository..."
  CWD=$(pwd)
  mkdir -p "$BIN_DIR"
  pushd /tmp
    git clone --depth 1 --branch "release-polkadot-$POLKADOT_V" https://github.com/paritytech/polkadot-sdk.git
    pushd polkadot-sdk
      echo "building polkadot executable..."
      cargo build --release --features fast-runtime
      cp target/release/polkadot "$CWD/$BIN_DIR"
      cp target/release/polkadot-execute-worker "$CWD/$BIN_DIR"
      cp target/release/polkadot-prepare-worker "$CWD/$BIN_DIR"
      cargo build --release -p polkadot-parachain-bin
      cp target/release/polkadot-parachain "$CWD/$BIN_DIR"
    popd
  popd
}

build_chainspec_generator() {
  echo "cloning chain-spec-generator..."
  CWD=$(pwd)
  mkdir -p "$BIN_DIR"
  pushd /tmp
    git clone https://github.com/polkadot-fellows/runtimes.git
    pushd runtimes
      echo "building chain-spec-generator..."
      cargo build --release --features fast-runtime
      cp target/release/chain-spec-generator "$CWD/$BIN_DIR"
    popd
  popd
}

fetch_polkadot() {
  echo "fetching from polkadot repository..."
  echo $BIN_DIR
  mkdir -p "$BIN_DIR"
  pushd "$BIN_DIR"
    wget https://github.com/paritytech/polkadot-sdk/releases/download/polkadot-$POLKADOT_V/polkadot
    wget https://github.com/paritytech/polkadot-sdk/releases/download/polkadot-$POLKADOT_V/polkadot-execute-worker
    wget https://github.com/paritytech/polkadot-sdk/releases/download/polkadot-$POLKADOT_V/polkadot-prepare-worker
    chmod +x *
  popd
}

zombienet_init() {
  if [ ! -f $ZOMBIENET_BIN ]; then
    echo "fetching zombienet executable..."
    curl -LO https://github.com/paritytech/zombienet/releases/download/$ZOMBIENET_V/$ZOMBIENET_BIN
    chmod +x $ZOMBIENET_BIN
  fi
  if [ ! -f $BIN_DIR/chain-spec-generator ]; then
    build_chainspec_generator
  fi
  if [ ! -f $BIN_DIR/polkadot ]; then
    if [ "$IS_LINUX" -eq 1 ]; then
        fetch_polkadot
    else
        build_polkadot
    fi
  fi
}

zombienet_build() {
  if [ ! -f $ZOMBIENET_BIN ]; then
    echo "fetching zombienet executable..."
    curl -LO https://github.com/paritytech/zombienet/releases/download/$ZOMBIENET_V/$ZOMBIENET_BIN
    chmod +x $ZOMBIENET_BIN
  fi
  if [ ! -f $BIN_DIR/polkadot ]; then
    build_polkadot
  fi
}

zombienet_testnet() {
  zombienet_init
  cargo build --release
  echo "spawning rococo-local relay chain plus mythos testnet as a parachain..."
  ./$ZOMBIENET_BIN spawn zombienet-config/testnet.toml -p native
}

zombienet_testnet_asset_hub() {
  zombienet_init
  cargo build --release
  echo "spawning rococo-local relay chain plus muse testnet as a parachain plus asset-hub..."
  ./$ZOMBIENET_BIN spawn zombienet-config/testnet-asset-hub.toml -p native
}

zombienet_mainnet() {
  zombienet_init
  cargo build --release
  echo "spawning polkadot-local relay chain plus mythos mainnet as a parachain..."
  ./$ZOMBIENET_BIN spawn zombienet-config/mainnet.toml -p native
}

zombienet_mainnet() {
  zombienet_init
  cargo build --release
  echo "spawning polkadot-local relay chain plus mythos mainnet as a parachain..."
  ./$ZOMBIENET_BIN spawn zombienet-config/mainnet.toml -p native
}

zombienet_mainnet_asset_hub() {
  zombienet_init
  cargo build --release
  echo "spawning polkadot-local relay chain plus mythos mainnet as a parachain plus asset-hub..."
  ./$ZOMBIENET_BIN spawn zombienet-config/mainnet-asset-hub.toml -p native
}


print_help() {
  echo "This is a shell script to automate the execution of zombienet."
  echo ""
  echo "$ ./zombienet.sh init                   # fetches zombienet and polkadot executables"
  echo "$ ./zombienet.sh build                  # builds polkadot executables from source"
  echo "$ ./zombienet.sh testnet                # spawns a rococo-local relay chain plus muse testnet-local as a parachain"
  echo "$ ./zombienet.sh testnet_asset_hub      # spawns a rococo-local relay chain plus muse testnet-local as a parachain plus asset-hub"
  echo "$ ./zombienet.sh mainnet                # spawns a polkadot-local relay chain plus mythos mainnet-local as a parachain"
  echo "$ ./zombienet.sh mainnet_asset_hub      # spawns a polkadot-local relay chain plus mythos mainnet-local as a parachain plus asset-hub"
}

SUBCOMMAND=$1
case $SUBCOMMAND in
  "" | "-h" | "--help")
    print_help
    ;;
  *)
    shift
    zombienet_${SUBCOMMAND} $@
    if [ $? = 127 ]; then
      echo "Error: '$SUBCOMMAND' is not a known SUBCOMMAND." >&2
      echo "Run './zombienet.sh --help' for a list of known subcommands." >&2
        exit 1
    fi
  ;;
esac
