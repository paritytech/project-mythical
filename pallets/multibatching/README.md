# Multibatching pallet

An alternative to standard batching utilities.

## Overview

The Multibatching pallet allows for an alternative approach to batching:
calls in a Multibatching batch can be made by multiple users, and their
approvals are collected off-chain. See docs for `batch()` for detailed
description.

## Dispatchable functions

- `batch()`: The batching function, allows making multiple calls by
  multiple users in a single transaction.
- `force_set_domain()`: Sets the domain for this specific pallet instance.
  Domain is a part of data that has to be signed by each caller in a batch,
  and is there to protect the users from replay attacks across networks.
