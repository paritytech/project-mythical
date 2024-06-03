# Migration Pallet

The Migration pallet facilitates the transfer of state from the Mythical Hyperledger Besu chain by providing essential functionalities. It enables the migration of collections, items, funds, and marketplace Asks.

## Overview

Within this pallet, a designated `Migrator` account is configured in the storage, granting permission to execute calls to pallet-nfts, pallet-marketplace, and pallet-balances to store necessary data:

-   `pallet-nfts`: Enables the creation of collections with specific IDs, configuration of roles for existing collections, setting metadata for collections, and minting items.
-   `pallet-marketplace`: Allows for the direct storage of Asks within the pallet's storage, eliminating the need for calling Marketplace::create_order().
-   `pallet-balances`: Facilitates the transfer of funds to a specified account using the balance of a preconfigured Pot account.

### **Runtime Requirement**

In order to be able to execute some priviledged operations to `pallet-nfts` the following configuration is required on the runtime:

```rust
pub type MigratorOrigin = EnsureSignedBy<pallet_migration::MigratorProvider<Runtime>, AccountId>;

impl pallet_nfts::Config for Runtime {
	...
	type ForceOrigin = MigratorOrigin;
	...
}
```

## Dispatchable Functions

-   `force_set_migrator`: Sets the migrator role, granting rights to call this pallet's extrinsics.
-   `set_next_collection_id`: Sets the NextCollectionId on pallet-nfts, to be used as the CollectionIdwhen the next collection is created.
-   `create_ask`: Creates an Ask inside the Marketplace pallet's storage
-   `send_funds_from_pot`: Transfer funds to a recipient account from the pot account.
-   `set_item_owner`: Transfers a given Nft to an AccountId.
-   `force_create`: Dispatches a call to pallet-nfts::force_create.
-   `set_team`: Dispatches a call to pallet-nfts::set_team.
-   `set_collection_metadata`: Dispatches a call to pallet-nfts::set_collection_metadata.
-   `force_mint`: Dispatches a call to pallet-nfts::force_mint.
-   `enable_serial_mint`: Modifies a collection config to set serial_mint = true.
