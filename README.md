# Mythical Testnet

### 🔰 Description

Mythical is a Web3 gaming platform. Building an NFT marketplace based on old chain (private Ethereum) Solidity Smart Contracts, integration with Mythical backend, bridge integration, and migrating chain state from old contracts.

### 🦀 Rust Setup

First, complete the [basic Rust setup instructions](./docs/rust-setup.md).

### 🔧 Build

Clone the parachain repository:

```sh
git clone https://github.com/paritytech/project-mythical
```

Use the following command to build the node without launching it:

```sh
cargo build --release
```

Or containerize with

```sh
docker build -t mythical-node --file ./docker/Dockerfile .
```

### 🕸️ Run a local network

You will have to use [Zombienet (available for Linux and MacOS)](https://github.com/paritytech/zombienet/releases) for spinning up a testnet, if you haven't setup zombienet yet, please refer to the [zombienet-setup](./.maintain/zombienet-setup.md) guide.

**To start a Development Network run:**

```sh
./zombienet.sh devnet # Starts a development network as specified in zombienet-config/devnet.toml
```

The script will take care of fetching the corresponding binaries for the relay chain.

**To start the Mainnet Network run:**

```sh
./zombienet.sh mainnet # Starts a development network as specified in zombienet-config/mainnet.toml
```

Currently this script will fail to start since the chain type needed to start the network was recently included in zombienet with the [following PR](https://github.com/paritytech/zombienet/pull/1699) and will be included on the next zombienet release.

In case the script fails to fetch the relay chain runtimes they can also be built from source using:

```sh
./zombienet.sh build
```
