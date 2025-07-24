# Mythos Parachain Node

### 🔰 Description

Parachain node for the Mythos Games blockchain platform.

### 🦀 Setup

First, complete the [basic Rust setup instructions](./docs/rust-setup.md).

### 🔧 Build

Clone the parachain repository:

```sh
git clone https://github.com/paritytech/project-mythical
```

Use the following command to build the node without launching it:

```sh
cargo build --profile release
```

Or containerize with

```sh
docker build -t mythos-node --file ./docker/Dockerfile .
```

### 🕸️ Run a local network

You will have to use [Zombienet (available for Linux and MacOS)](https://github.com/paritytech/zombienet/releases) for spinning up a testnet, if you haven't setup zombienet yet, please refer to the [zombienet-setup](./.maintain/zombienet-setup.md) guide.

**To start a Development Network run:**

```sh
./zombienet.sh testnet # Starts a development network as specified in zombienet-config/testnet.toml
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

### Notes regarding presets

Presets are predefined configurations that simplify initializing a chain’s genesis state. They
establish key parameters like balances, collator sets, council members, and the sudo (root) account.
By using presets, you can quickly spin up a network for testing or specialized use without manually
defining every genesis detail.

#### dev

- Minimal, local-friendly preset with default collators (e.g., “Alith” and “Balthasar”).
- Distributes tokens across several accounts for testing.
- Sets a small council and a single sudo key for convenient local control.

#### muse/mythos

- Uses two specific collators.
- Assigns different account balances for a more structured distribution.
- No council members; includes one sudo key for administration.

These presets are especially useful for quickly preparing Zombienet-based test setups or any local
development scenario without manually customizing every detail.

### Notes regarding Polkadot.js and Ethereum accounts

On Polkadot.js, when bootstraping the local network with zombienet, the normal accounts are not derived from a seed.
Instead, there is a [known set of private/public keypairs](https://github.com/polkadot-js/common/blob/e3b11360ab381a51bb05544cf2872646037d3579/packages/keyring/src/testing.ts#L76) that are used instead.

In normal circumstances when interacting with the GUI it is not necessary to control these private keys when submitting extrinsics to the chain, but if you are interacting via script then they are needed.
