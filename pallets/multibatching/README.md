# Multibatching pallet

An alternative to standard batching utilities.

## Overview

The Multibatching pallet allows for an alternative approach to batching:
calls in a Multibatching batch can be made by multiple users, and their
approvals are collected off-chain. See docs for `batch()` and `batch_v2()`
for detailed description.

## Dispatchable functions

- `batch()`: The batching function, allows making multiple calls by
  multiple users in a single transaction.
- `batch_v2()`: The batching function, allows making multiple calls by
  multiple users in a single transaction.
