# Proxy Module

## Overview

The Proxy module allows accounts to give permission to other accounts to make calls on their behalf. This module is useful for delegating tasks, managing resources, and enhancing security through proxy accounts.

### Main Entities

- **Delegator**: The account that gives permission to another account to make calls on its behalf.
- **Delegate**: The account that is given permission to make calls on behalf of the delegator.
- **Sponsor**: An account that can pay the deposit for the proxy. Sponsors have permission to remove proxies that they have paid the deposit for. It should be a secure cold wallet.
- **Sponsor Agent**: An account authorized by the Sponsor to initiate the funding of proxies using the Sponsor’s resources. This role is designed to facilitate transactions while minimizing direct exposure of the Sponsor’s credentials.

## Unique Features

This Proxy module offers several unique features designed for scenarios where accounts need to delegate permissions securely:

1. **Sponsor-Supported Proxies**:
   - Sponsors can reserve deposits for proxies, which helps manage funds securely without requiring pre-funding of the delegator's account.
   - Sponsors can remove proxies they have sponsored, ensuring control over their reserved funds and preventing loss of funds if the proxy is removed.
   - Useful in scenarios where an organization creates accounts for users and needs to manage funds centrally.

2. **Immutable Proxy Settings by Delegate**:
   - Delegates cannot modify proxy settings for the delegator.
   - Even if the delegate's private key is compromised, they cannot change the proxy settings, thereby securing the delegator's account from being fully compromised.
   - The delegate can perform token transfers but cannot assign a new proxy or revoke the current one, maintaining the integrity of the delegator's access.
   - This is crucial when the delegator's private key is discarded after setting up proxy access, ensuring that control over the account remains intact and cannot be transferred.

3. **Sponsor-Controlled Proxy Removal**:
   - Sponsors can delete proxies they have sponsored.
   - This allows sponsors to reclaim their reserved funds if the proxy access is no longer needed or if the account needs to be deactivated.
   - Ensures that only the sponsor or the original delegator can revoke access, preventing unauthorized changes.

4. **Error Propagation in Proxied Calls**:
   - Errors from the proxied call are propagated back through the `proxy` method.
   - This allows the Proxy module to be used seamlessly within the `utility.batchAll()` method, enabling atomic batch transactions that include proxy calls.
   - Ensures that any issues in proxied transactions are correctly reported and handled, facilitating robust transaction management within batch operations.

## Usage

### Extrinsics

#### `add_proxy`

Adds a new proxy, allowing a delegator to grant permission to a delegate account to act on their behalf for a specific subset of calls defined by `proxy_type`. Optionally, a sponsor can be specified who will reserve the deposit required for the proxy. The reserved deposit is returned when the proxy is removed.

#### `proxy`

Executes a call on behalf of the delegator, provided the delegate has the appropriate proxy permission. The call must be within the subset of allowed calls defined by the proxy type. Errors from the proxied call are propagated back.

#### `remove_proxy`

Removes an existing proxy, allowing a delegator to revoke the permission previously granted to a delegate. If a sponsor was specified during the proxy creation, the reserved deposit is returned to the sponsor.

#### `approve_proxy_funding`

Allows a sponsor agent to approve the reservation of funds for a proxy on behalf of the sponsor. The approval must be given before the proxy can be created using the sponsor's funds.

#### `register_sponsor_agent`

Registers an agent who is authorized to approve the reservation of funds for proxies on behalf of the sponsor. This helps in delegating the responsibility of managing proxy fund reservations while keeping the sponsor's credentials secure.

#### `revoke_sponsor_agent`

Revokes the authorization of a sponsor agent. Once revoked, the agent will no longer be able to approve the reservation of funds for proxies on behalf of the sponsor. All previously approved fund reservations by this agent that have not yet been used to create proxies will also be invalidated. Existing proxies created with the agent's approval will remain unaffected.


#### `remove_sponsored_proxy`

Allows a sponsor to remove a proxy that they have sponsored. The reserved deposit is returned to the sponsor upon removal of the proxy.

## Events

- `ProxyCreated`: A new proxy permission was added.
- `ProxyRemoved`: A proxy permission was removed.
- `ProxySponsorshipApproved`: Proxy funding was approved.
- `SponsorAgentRegistered`: A sponsor agent was registered.
- `SponsorAgentRevoked`: A sponsor agent was revoked.
- `ProxyExecuted`: A proxy call was executed.

## Hooks

- `on_idle`: Cleans up approvals that are no longer valid because the agent has been removed.
