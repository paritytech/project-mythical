#![cfg_attr(not(feature = "std"), no_std)]
use core::marker::PhantomData;
use frame_support::{
	traits::fungible::{Inspect, Mutate},
	weights::{constants::WEIGHT_REF_TIME_PER_SECOND, Weight},
};
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_core::{crypto::FromEntropy, U256};

use sp_runtime::{
	traits::{IdentifyAccount, Verify},
	Perbill,
};

pub use account::EthereumSignature;
use frame_support::traits::Incrementable;

// Cumulus types re-export
// These types are shared between the mainnet and testnet runtimes
//https://github.com/paritytech/cumulus/tree/master/parachains/common
pub use parachains_common::{AuraId, Balance, Block, BlockNumber, Hash};

pub type Signature = EthereumSignature;

/// Use AccountId20 for Ethereum address
pub type AccountId = <<Signature as Verify>::Signer as IdentifyAccount>::AccountId;

pub type AccountIdOf<R> = <R as frame_system::Config>::AccountId;

/// Nonce for an account
pub type Nonce = u32;

/// This determines the average expected block time that we are targeting.
/// Blocks will be produced at a minimum duration defined by `SLOT_DURATION`.
/// `SLOT_DURATION` is picked up by `pallet_timestamp` which is in turn picked
/// up by `pallet_aura` to implement `fn slot_duration()`.
///
/// Change this to adjust the block time.
pub const MILLISECS_PER_BLOCK: u64 = 6000;

// NOTE: Currently it is not possible to change the slot duration after the chain has started.
// Attempting to do so will brick block production.
pub const SLOT_DURATION: u64 = MILLISECS_PER_BLOCK;

// Time is measured by number of blocks.
pub const MINUTES: BlockNumber = 60_000 / (MILLISECS_PER_BLOCK as BlockNumber);
pub const HOURS: BlockNumber = MINUTES * 60;
pub const DAYS: BlockNumber = HOURS * 24;

/// We assume that ~5% of the block weight is consumed by `on_initialize` handlers. This is
/// used to limit the maximal weight of a single extrinsic.
pub const AVERAGE_ON_INITIALIZE_RATIO: Perbill = Perbill::from_percent(5);

/// We allow `Normal` extrinsics to fill up the block up to 75%, the rest can be used by
/// `Operational` extrinsics.
pub const NORMAL_DISPATCH_RATIO: Perbill = Perbill::from_percent(75);

/// Max block weight configuration.
pub const MAXIMUM_BLOCK_WEIGHT: Weight = Weight::from_parts(
	WEIGHT_REF_TIME_PER_SECOND.saturating_mul(2),
	polkadot_primitives::MAX_POV_SIZE as u64,
);

#[derive(Clone, TypeInfo, Encode, PartialEq, Eq, Decode, Copy, MaxEncodedLen, Debug)]
pub struct IncrementableU256(U256);

impl Incrementable for IncrementableU256 {
	fn increment(&self) -> Option<Self> {
		let val = self.clone();
		Some(Self(val.0.saturating_add(U256::one())))
	}

	fn initial_value() -> Option<Self> {
		Some(Self(U256::zero()))
	}
}

//Needed for Pallet Nfts Benchmark Helper
impl From<u16> for IncrementableU256 {
	fn from(value: u16) -> Self {
		IncrementableU256(U256::from(value))
	}
}

pub struct TreasuryBenchmarkHelper<T>(PhantomData<T>);
impl<T> pallet_treasury::ArgumentsFactory<(), AccountId> for TreasuryBenchmarkHelper<T>
where
	T: Mutate<AccountId> + Inspect<AccountId>,
{
	fn create_asset_kind(_seed: u32) -> () {
		()
	}
	fn create_beneficiary(seed: [u8; 32]) -> AccountId {
		let account = AccountId::from_entropy(&mut seed.as_slice()).unwrap();
		<T as Mutate<_>>::mint_into(&account, <T as Inspect<_>>::minimum_balance()).unwrap();
		account
	}
}
