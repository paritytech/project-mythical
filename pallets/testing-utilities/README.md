# Testing Utilities Pallet

This pallet is intended to contain utilities for simulating behaviors on mainnet
that are otherwise hard to achieve in testnets, especially local.

## Overview

Currently, the pallet only contains one method: `transfer_through_delayed_remint`,
that performs an equivalent of `transfer_keep_alive` in `pallet_balances` but with different set of
events.

## Extrinsics

### Transfer Through Delayed Re-Mint

Schedules an operation to be performed in block's on_idle hook, that transfers
the required `amount` from the source wallet `from` to the destination wallet
`to`, that burns assets on source and mints them from the destination. The
corresponding events are generated that are not attached to the transaction
that originated the operation.

## Building and Testing

To test this pallet, you can use the following command:

```bash
# Run tests
cargo test
```
