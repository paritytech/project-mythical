# Escrow Balances Pallet

This Substrate pallet implements escrow balances, providing a framework for managing funds held in escrow. It allows users to deposit funds into specific accounts with a designated agent, who can perform operations such as releasing and revoking deposits.

## Overview

The pallet enables a simple escrow system with the following operations:

- **Deposit**: Place funds into an escrow account, designating an authorised agent to manage the deposit.
- **Release**: Release a specific amount from the escrow account, typically when certain conditions are met.
- **Revoke**: Revoke a deposit and transfer funds to another account.

## Extrinsics

### Deposit

The `deposit` extrinsic allows users to deposit funds into an escrow account. It requires the following parameters:

- `origin`: The originator of the deposit. It must be signed.
- `address`: The account into which the funds are deposited.
- `value`: The amount to be deposited.
- `authorised_agent`: The agent authorised to manage the deposit.

### Release

The `release` extrinsic enables releasing funds from an escrow account. It requires:

- `origin`: The originator of the release. It must be signed.
- `address`: The account from which the funds are released.
- `value`: The amount to be released.

### Revoke

The `revoke` extrinsic allows revoking a deposit and transferring funds to another destination. It requires:

- `origin`: The originator of the revocation. It must be signed.
- `address`: The account where the funds are held.
- `destination`: The account to which the funds are transferred.
- `reason`: A reason for the revocation, provided as a byte array.

## Building and Testing

To test this pallet, you can use the following command:

```bash
# Run tests
cargo test
```
