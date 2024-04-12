#![cfg_attr(not(feature = "std"), no_std)]
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_core::U256;
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_core::U256;
use sp_runtime::{
	traits::{IdentifyAccount, Verify},
	Perbill,
};

pub use account::EthereumSignature;
use frame_support::traits::{Currency, Imbalance, Incrementable, OnUnbalanced};
use pallet_balances::NegativeImbalance;
use sp_std::marker::PhantomData;

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

/// We assume that ~5% of the block weight is consumed by `on_initialize` handlers. This is
/// used to limit the maximal weight of a single extrinsic.
pub const AVERAGE_ON_INITIALIZE_RATIO: Perbill = Perbill::from_percent(5);

/// We allow `Normal` extrinsics to fill up the block up to 75%, the rest can be used by
/// `Operational` extrinsics.
pub const NORMAL_DISPATCH_RATIO: Perbill = Perbill::from_percent(75);

/// Implementation of `OnUnbalanced` that deals with the fees by combining tip and fee and passing
/// the result on to `ToStakingPot`.
pub struct DealWithFees<R>(PhantomData<R>);
impl<R> OnUnbalanced<NegativeImbalance<R>> for DealWithFees<R>
where
	R: pallet_balances::Config + pallet_collator_selection::Config,
	AccountIdOf<R>: From<account::AccountId20> + Into<account::AccountId20>,
	<R as frame_system::Config>::RuntimeEvent: From<pallet_balances::Event<R>>,
{
	fn on_unbalanceds<B>(mut fees_then_tips: impl Iterator<Item = NegativeImbalance<R>>) {
		if let Some(mut fees) = fees_then_tips.next() {
			if let Some(tips) = fees_then_tips.next() {
				tips.merge_into(&mut fees);
			}
			<ToStakingPot<R> as OnUnbalanced<_>>::on_unbalanced(fees);
		}
	}
}

/// Implementation of `OnUnbalanced` that deposits the fees into a staking pot for later payout.
pub struct ToStakingPot<R>(PhantomData<R>);
impl<R> OnUnbalanced<NegativeImbalance<R>> for ToStakingPot<R>
where
	R: pallet_balances::Config + pallet_collator_selection::Config,
	AccountIdOf<R>: From<account::AccountId20> + Into<account::AccountId20>,
	<R as frame_system::Config>::RuntimeEvent: From<pallet_balances::Event<R>>,
{
	fn on_nonzero_unbalanced(amount: NegativeImbalance<R>) {
		let staking_pot = <pallet_collator_selection::Pallet<R>>::account_id();
		<pallet_balances::Pallet<R>>::resolve_creating(&staking_pot, amount);
	}
}

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
