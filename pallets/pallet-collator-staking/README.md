# Collator Staking Pallet

Simple DPoS pallet for staking and managing collators in a Polkadot parachain.

## Overview

The Collator Staking pallet is an extension of
the [Cumulus Collator Selection pallet](https://github.com/paritytech/polkadot-sdk/tree/master/cumulus/pallets/collator-selection)
and provides DPoS functionality to manage collators of a parachain.

It allows users to stake their tokens to back collators, and receive rewards proportionately.
There is no slashing in place. If a collator does not produce blocks as expected, they are removed from the collator set
and all stake is refunded.

## Implementation

Similar to the Collator Selection pallet, this pallet also maintains two kind of block producers:

- `Invulnerables`: accounts that are always selected to become collators. They can only be removed by the pallet's
  authority. Invulnerables do not receive staking rewards.
- `Candidates`: accounts that compete to be part of the collator set based on delegated stake.

### Rewards

Staking rewards distributed to candidates and their stakers come from the following sources:

- Transaction fees and tips collected for blocks produced.
- An optional per-block flat amount coming from a different pot. This is to "top-up" the rewards
  in case fees and tips are too small.

The pallet assumes all rewards are generated from existing funds on the blockchain, and **there is no inflation**
implemented as part of this pallet.

Rewards are distributed so that all stakeholders are incentivized to participate:

- Candidates compete to become collators.
- Collators must not misbehave and produce blocks honestly so that they increase the chances to produce more blocks and
  this way be more attractive for other users to stake on.
- Stakers must select wisely the candidates they want to deposit the stake on, hence determining the best possible
  candidates that are likely to become collators.
- Rewards are proportionally distributed among collators and stakers when the session ends.
    - Collators receive an exclusive percentage of them for collating. This is configurable.
    - Stakers receive the remaining proportionally to the amount staked in a given collator.

### Staking

Any account in the parachain can deposit a stake on a given candidate so that it gets access to staking rewards
proportional to the blocks produced by the selected candidate.

### Un-staking

When a staker wishes to unstake, there is a delay: the staker will have to wait for a given number of blocks
before their funds are unlocked. No rewards are given during this delay period.

### Auto Compounding

Users can also select the percentage of rewards that will be auto-compounded. If the selected percentage is greater than
zero, part of the rewards will be re-invested as stake in the collator when receiving rewards per block.

### Reward delivery

When a session ends, a snapshot of the generated rewards and the per-candidate stake is taken. This information is used
to calculate the per-staker rewards for that session.
Stakers will have to call the `claim_rewards` extrinsic to collect the corresponding earnings from the previous
sessions. This process will be automatically triggered whenever
the staker alter its stake. That is: when `stake`, `unstake` or `unstake_all` is called. Claimed rewards will then be
auto-compounded if the user sets an auto-compound percentage greater than zero.

### Runtime Configuration

| Parameter                  | Description                                                                                          |
|----------------------------|------------------------------------------------------------------------------------------------------|
| `RuntimeEvent`             | The overarching event type.                                                                          |
| `Currency`                 | The currency mechanism.                                                                              |
| `RuntimeFreezeReason`      | The overarching freeze reason.                                                                       |
| `UpdateOrigin`             | Origin that can dictate updating parameters of this pallet.                                          |
| `PotId`                    | Account Identifier from which the internal pot is generated.                                         |
| `ExtraRewardPotId`         | Account Identifier from which the internal extra reward pot is generated.                            |
| `ExtraRewardReceiver`      | Account that will receive all funds in the extra reward pot when those are stopped.                  |
| `MaxCandidates`            | Maximum number of candidates allowed.                                                                |
| `MinEligibleCollators`     | Minimum number eligible collators including Invulnerables.                                           |
| `MaxInvulnerables`         | Maximum number of invulnerables.                                                                     |
| `KickThreshold`            | Candidates will be removed from active collator set, if block is not produced within this threshold. |
| `CollatorId`               | A stable ID for a collator.                                                                          |
| `CollatorIdOf`             | A conversion from account ID to collator ID.                                                         |
| `CollatorRegistration`     | Validate a collator is registered.                                                                   |
| `MaxStakedCandidates`      | Maximum candidates a staker can stake on.                                                            |
| `MaxStakers`               | Maximum stakers per candidate.                                                                       |
| `StakeUnlockDelay`         | Number of blocks to wait before unlocking the stake by a user.                                       |
| `BondUnlockDelay`          | Number of blocks to wait before unlocking the bond by a collator.                                    |
| `RestakeUnlockDelay`       | Number of blocks to wait before reusing funds previously assigned to a candidate.                    |
| `MaxRewardSessions`        | Maximum number of per-session reward snapshots to keep in storage.                                   |
| `AutoCompoundingThreshold` | Minimum stake needed to enable autocompounding.                                                      |
| `WeightInfo`               | Information on runtime weights.                                                                      |

### Setup Considerations

While it is desired to set `MaxStakedCandidates` and `MaxStakers` to a reasonably high value, bear in mind this may
significantly impact block weight consumption. We recommend to measure the weights and set values that in the worst case
do not occupy more than a sane limit, like ~10% of the block's total weight.

### Dependencies

#### Pallet Session

This pallet is coupled
with [pallet-session](https://github.com/paritytech/polkadot-sdk/tree/master/substrate/frame/session), as it plays the
role of the session manager by deciding the next collator set. Also, rewards are assigned when sessions end and start.

#### Pallet Authorship

This pallet is dependent
on [pallet-authorship](https://github.com/paritytech/polkadot-sdk/tree/master/substrate/frame/authorship) by subscribing
to block authorship so that it can assign rewards to collators and their stakers accordingly.

### Compatibility

This pallet is compatible
with [polkadot release stable2407-2](https://github.com/paritytech/polkadot-sdk/releases/tag/polkadot-stable2407-2) or
higher.

## License

The code within this repository is licensed under Apache-2.0 license. See the [LICENSE](./LICENSE) file for more
details.
