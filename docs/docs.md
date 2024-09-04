# Mythos Parachain Documentation

## Overview

[Mythos](https://mythos.foundation/) is an innovative gaming platform designed to revolutionize the gaming industry.
It aims to democratize the gaming world by enabling both players and creators to actively participate in and benefit from the value chain.
The platform supports multi-chain ecosystems, unified marketplaces, decentralized financial systems, decentralized governance mechanisms, and multi-token game economies.

This document provides an overview of the Mythos parachain and the $MYTH token, including its runtimes, bootstrapping methods, integrations, and custom pallets.

## $MYTH token

The ([\$MYTH token](https://www.coinbase.com/en-es/price/mythos)) is an interoperable utility token that seeks to simplify, standardize, and democratize Web3 gaming.
It is designed with the intention of providing opportunities for anyone to participate and contribute within the ecosystem, adding governance and value to game developers, publishers, and content creators.
The $MYTH token is an integral part of the Mythos Foundation and the Mythos Blockchain Ecosystem DAO, which are focused on reducing barriers to entry for innovative game developers and expanding the reach of web3-based interactive experiences.

### Migration from Ethereum to Polkadot via Snowbridge

Originally, the $MYTH token was live on the Ethereum mainnet. It has since been bridged to the Polkadot network via **[Snowbridge](https://docs.snowbridge.network/)**, enhancing its interoperability and integration within the Mythos ecosystem.


## Runtimes

- **Muse ([Testnet](../runtime/testnet/src/lib.rs))**:
	- Deployed on [Rococo](https://dotapps-io.ipns.dweb.link/?rpc=wss%3A%2F%2Frococo-muse-rpc.polkadot.io#/explorer) and [Paseo](https://dotapps-io.ipns.dweb.link/?rpc=wss%3A%2F%2Fpaseo-muse-rpc.polkadot.io#/explorer) networks.
	- Used for testing and development purposes.

- **Mythos ([Mainnet](../runtime/mainnet/src/lib.rs))**:
	- Deployed on the [Polkadot](https://dotapps-io.ipns.dweb.link/?rpc=wss%3A%2F%2Fpolkadot-mythos-rpc.polkadot.io#/explorer) network.
	- Handles live transactions and operations.


## Polkadot Version

The [Mythos parachain](https://parachains.info/details/mythos) operates on the latest stable release of Polkadot.


## Local Blockchain Bootstrapping

To bootstrap a local instance of the blockchain, use the script [./zombienet.sh](../zombienet.sh) with the `testnet` or `mainnet` option, depending on the environment you wish to set up.

```bash
### Testnet - Muse local network.
./zombienet.sh testnet

### Mainnet - Mythos local network.
./zombienet.sh mainnet
```

## XCM Channels

Mythos is currently integrated with the following HRMP (Horizontal Relay-Chain Messaging Protocol) channels:

- **[AssetHub](https://parachains.info/details/assethub_polkadot)**: For asset management and transfers.
- **[HydraDX](https://parachains.info/details/hydration)**: For decentralized liquidity provision and swaps.


## Pallets

### FRAME pallets

The Mythos parachain includes the following standard FRAME pallets:

1. **[pallet-balances](https://crates.io/crates/pallet-balances)**
    - **Purpose**: Manages the balances of accounts, handling transfers, and ensuring account balance integrity of the $MYTH token.

2. **[pallet-transaction-payment](https://crates.io/crates/pallet-transaction-payment)**
    - **Purpose**: Handles transaction fees and payment mechanisms.

3. **[pallet-sudo](https://crates.io/crates/pallet-sudo)**
    - **Purpose**: Provides administrative privileges for executing privileged operations. It is planned to be phased out in favor of decentralized governance mechanisms.

4. **[pallet-collective](https://crates.io/crates/pallet-collective)**
    - **Purpose**: Manages the council for collective decision-making and governance.

5. **[pallet-authorship](https://crates.io/crates/pallet-authorship)**
    - **Purpose**: Supports block authorship and validation.

6. **[pallet-collator-selection](https://crates.io/crates/pallet-collator-selection)**
    - **Purpose**: Handles the selection and management of collators.

7. **[pallet-session](https://crates.io/crates/pallet-session)**
   - **Purpose**: Manages session-related functionality, including the management of validators.

8. **[pallet-aura](https://crates.io/crates/pallet-aura)**
   - **Purpose**: Provides the Aura consensus mechanism for block production.

9. **[cumulus-pallet-aura-ext](https://crates.io/crates/cumulus-pallet-aura-ext)**
   - **Purpose**: Extends the functionality of Aura for enhanced features.

10. **[pallet-xcm](https://crates.io/crates/pallet-xcmhttps://crates.io/crates/pallet-xcm)**
    - **Purpose**: Facilitates cross-chain communication and interactions.

11. **[pallet-xcmp-queue](https://crates.io/crates/cumulus-pallet-xcmp-queue)**
    - **Purpose**: Manages the queue for cross-chain messages.

12. **[cumulus-pallet-xcm](https://crates.io/crates/cumulus-pallet-xcm)**
    - **Purpose**: Provides additional XCM functionalities for Cumulus.

13. **[pallet-message-queue](https://crates.io/crates/pallet-message-queue)**
    - **Purpose**: Handles the queuing of cross-chain messages.

14. **[pallet-proxy](https://crates.io/crates/pallet-proxy)**
    - **Purpose**: Allows for account delegation and proxying of calls.

15. **[pallet-vesting](https://crates.io/crates/pallet-vesting)**
    - **Purpose**: Manages the vesting of tokens over time.

16. **[pallet-utility](https://crates.io/crates/pallet-utility)**
    - **Purpose**: Helpers for dispatch management such as transaction batching.


### Custom pallets

The Mythos parachain includes several custom pallets that enhance its functionality:

1. **[pallet-dmarket](../pallets/dmarket/src/lib.rs)**
	- **Description**: Provides a marketplace for buying and selling NFTs.
	- **Functionality**: Manages trading NFTs through the pallet-nfts.

2. **[pallet-escrow](../pallets/escrow/src/lib.rs)**
	- **Description**: Implements a framework for managing funds held in escrow.
	- **Functionality**: Ensures secure handling of transactions by holding funds until conditions are met.

3. **[pallet-marketplace](../pallets/marketplace/src/lib.rs)**
	- **Description**: Facilitates a marketplace for NFTs using Asks and Bids.
	- **Functionality**: Enables users to buy and sell NFTs from the pallet-nfts.

4. **[pallet-multibatching](../pallets/multibatching/src/lib.rs)**
	- **Description**: Offers an alternative approach to batching.
	- **Functionality**: Allows multiple users to make calls in a batch, with off-chain approval collection.

5. **[pallet-nfts](../pallets/nfts/src/lib.rs)**
	- **Description**: A fork of the original pallet-nfts.
	- **Functionality**: Provides additional functionalities not available in the original pallet-nfts.

6. **[pallet-myth-proxy](../pallets/myth-proxy/src/lib.rs)**
	- **Description**: A proxy module that allows account delegation.
	- **Functionality**: Enhances security and resource management by enabling accounts to delegate tasks to other accounts.


## Technical details

### AccountId20 and EthereumSignature Support

The Mythos parachain uses [**AccountId20** and **EthereumSignature**](../primitives/account/src/lib.rs) to maintain compatibility with Ethereum-based systems.

- **AccountId20**: This is a 20-byte (160-bit) address format, similar to Ethereum addresses.
  This compatibility allows Mythos to integrate with Ethereum wallets and services, providing a familiar experience for users migrating from or interacting with Ethereum-based platforms.

- **EthereumSignature**: Mythos utilizes Ethereum's signature scheme to verify the authenticity of transactions signed by Ethereum accounts.
  This is particularly important for cross-chain asset transfers and interactions where Ethereum accounts are involved.
  By supporting Ethereum signatures, Mythos enhances its ability to interact with Ethereum-based decentralized applications (dApps) and services.

### XCM Configuration

The Mythos parachain leverages the **[XCM (Cross-Consensus Messaging)](https://wiki.polkadot.network/docs/learn/xcm)** framework to facilitate cross-chain communication and asset transfers within the Polkadot ecosystem and beyond.

- **Integration with [AssetHub](https://parachains.info/details/assethub)**: AssetHub acts as a centralized point for managing and transferring assets across parachains.
  Mythos’ integration with AssetHub ensures that users can easily transfer assets between Mythos and other parachains in the Polkadot ecosystem.

- **[Snowbridge](https://wiki.polkadot.network/docs/general/dune-analytics/snowbridge-dashboards) and Ethereum Integration**:
  Snowbridge provides a trustless bridge between Polkadot and Ethereum, allowing assets and data to flow seamlessly between the two networks.
  Mythos’ integration with Snowbridge means it can receive and manage the [original $MYTH](https://etherscan.io/address/0xba41ddf06b7ffd89d1267b5a93bfef2424eb2003) token originally minted on Ethereum, facilitating interoperability with the broader Ethereum ecosystem.

The XCM configuration on Mythos ensures secure, efficient, and scalable cross-chain operations, allowing for robust interactions across various chains and networks.


### Benchmarking Hardware

The standard [hardware setup](https://wiki.polkadot.network/docs/maintain-guides-how-to-validate-polkadot#reference-hardware) for Polkadot validators is used for benchmarking Mythos. This typically includes:

- **CPU**: 8 cores, 16 threads.
- **RAM**: 32 GB.
- **Storage**: NVMe SSD, 1 TB.
- **Network**: High-speed internet connection with low latency.

This configuration ensures optimal performance and reliability for validating and collating on the Mythos parachain.

### Consensus Details

#### Collator Selection
- Mythos uses the **[Collator Selection](https://crates.io/crates/pallet-collator-selection)** pallet to manage the selection and management of collators who produce blocks and validate transactions.
  This pallet ensures that the selection process is fair and transparent, providing incentives for collators to act honestly.
- **Collators**: These are nodes that gather transactions, create blocks, and submit them to the relay chain for finalization. The selection process is designed to be transparent and fair, incentivizing collators to act in the best interest of the network. Collators are rewarded for their participation, encouraging them to maintain high availability and performance.

#### Aura
- **[Aura](https://crates.io/crates/pallet-aura)** is a Proof-of-Authority (PoA) consensus algorithm where a set of pre-approved authorities (collators) are responsible for producing blocks in a round-robin fashion.
  This ensures that blocks are produced regularly and efficiently, with minimal delays between them.
- **Security and Efficiency**: Aura is known for its low-latency block production, which is critical for applications requiring quick transaction confirmations, such as gaming platforms.
  Aura’s deterministic nature helps prevent forks and ensures that the blockchain progresses smoothly.


### Finality and Average Time to Finality

- **Finality**: The Mythos parachain achieves finality through the Polkadot network's [shared security model](https://wiki.polkadot.network/docs/learn-parachains#parachain-benefits), which provides robust and reliable finality guarantees.

- **Average Time to Finality**: The Mythos blockchain uses [asynchronous backing](https://wiki.polkadot.network/docs/learn-async-backing), which implies that the average time to finality on Mythos is approximately **6 seconds**, which is consistent with the Polkadot network's performance metrics and ensures rapid confirmation of transactions, usually within **18 seconds**.
