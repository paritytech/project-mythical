#![cfg_attr(not(feature = "std"), no_std)]
// `construct_runtime!` does a lot of recursion and requires us to increase the limit to 256.
#![recursion_limit = "256"]

// Make the WASM binary available.
#[cfg(feature = "std")]
include!(concat!(env!("OUT_DIR"), "/wasm_binary.rs"));

pub mod apis;
pub mod configs;
pub mod genesis_config_presets;
mod weights;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarks;

extern crate alloc;
pub use fee::WeightToFee;

use cumulus_primitives_core::AssetId;

use sp_core::{crypto::KeyTypeId, OpaqueMetadata};

use sp_runtime::{
	generic, impl_opaque_keys,
	traits::BlakeTwo256,
	transaction_validity::{TransactionSource, TransactionValidity},
	ApplyExtrinsicResult, ExtrinsicInclusionMode,
};

use sp_std::prelude::*;
#[cfg(feature = "std")]
use sp_version::NativeVersion;
use sp_version::RuntimeVersion;

use frame_support::genesis_builder_helper::{build_state, get_preset};
use pallet_dmarket::{Item, TradeParams};
use polkadot_primitives::Moment;
pub use runtime_common::{
	AccountId, AccountIdOf, Balance, BlockNumber, Hash, IncrementableU256, Nonce, Signature,
	AVERAGE_ON_INITIALIZE_RATIO, DAYS, HOURS, MAXIMUM_BLOCK_WEIGHT, MINUTES, NORMAL_DISPATCH_RATIO,
	SLOT_DURATION,
};
pub use sp_consensus_aura::sr25519::AuthorityId as AuraId;
pub use sp_runtime::{MultiAddress, Perbill, Permill};
use xcm::{prelude::XcmVersion, VersionedAssets};
use xcm_runtime_apis::{
	dry_run::{CallDryRunEffects, Error as XcmDryRunApiError, XcmDryRunEffects},
	fees::Error as XcmPaymentApiError,
};

#[cfg(any(feature = "std", test))]
pub use sp_runtime::BuildStorage;

// Polkadot imports

use weights::ExtrinsicBaseWeight;

/// The address format for describing accounts.
pub type Address = AccountId;

/// Block header type as expected by this runtime.
pub type Header = generic::Header<BlockNumber, BlakeTwo256>;

/// Block type as expected by this runtime.
pub type Block = generic::Block<Header, UncheckedExtrinsic>;

/// A Block signed with a Justification
pub type SignedBlock = generic::SignedBlock<Block>;

/// BlockId type as expected by this runtime.
pub type BlockId = generic::BlockId<Block>;

/// The extension to the basic transaction logic.
pub type TxExtension = cumulus_pallet_weight_reclaim::StorageWeightReclaim<
	Runtime,
	(
		frame_system::CheckNonZeroSender<Runtime>,
		frame_system::CheckSpecVersion<Runtime>,
		frame_system::CheckTxVersion<Runtime>,
		frame_system::CheckGenesis<Runtime>,
		frame_system::CheckEra<Runtime>,
		frame_system::CheckNonce<Runtime>,
		frame_system::CheckWeight<Runtime>,
		pallet_transaction_payment::ChargeTransactionPayment<Runtime>,
		frame_metadata_hash_extension::CheckMetadataHash<Runtime>,
	),
>;

/// Unchecked extrinsic type as expected by this runtime.
pub type UncheckedExtrinsic =
	generic::UncheckedExtrinsic<Address, RuntimeCall, Signature, TxExtension>;

/// Extrinsic type that has already been checked.
pub type CheckedExtrinsic = generic::CheckedExtrinsic<AccountId, RuntimeCall, TxExtension>;

/// Pending migrations to be applied.
pub type Migrations = ();

/// Executive: handles dispatch to the various modules.
pub type Executive = frame_executive::Executive<
	Runtime,
	Block,
	frame_system::ChainContext<Runtime>,
	Runtime,
	AllPalletsWithSystem,
	Migrations,
>;

pub mod fee {
	use super::{Balance, ExtrinsicBaseWeight, MILLI_DOT, MILLI_MYTH};
	use frame_support::weights::{
		constants::WEIGHT_REF_TIME_PER_SECOND, FeePolynomial, Weight, WeightToFeeCoefficient,
		WeightToFeeCoefficients, WeightToFeePolynomial,
	};
	use smallvec::smallvec;
	use sp_runtime::Perbill;

	/// This constant will multiply the overall fee users will have to spend for transactions.
	pub const FEE_MULTIPLIER: Balance = 7;

	/// Handles converting a weight scalar to a fee value, based on the scale and granularity of the
	/// node's balance type.
	///
	/// This should typically create a mapping between the following ranges:
	///   - `[0, MAXIMUM_BLOCK_WEIGHT]`
	///   - `[Balance::min, Balance::max]`
	///
	/// Yet, it can be used for any other sort of change to weight-fee. Some examples being:
	///   - Setting it to `0` will essentially disable the weight fee.
	///   - Setting it to `1` will cause the literal `#[weight = x]` values to be charged.
	pub struct WeightToFee;
	impl frame_support::weights::WeightToFee for WeightToFee {
		type Balance = Balance;

		fn weight_to_fee(weight: &Weight) -> Self::Balance {
			let ref_polynomial: FeePolynomial<Balance> = RefTimeToFee::polynomial().into();
			let proof_polynomial: FeePolynomial<Balance> = ProofSizeToFee::polynomial().into();

			// Get fee amount from ref_time based on the RefTime polynomial
			let ref_fee: Balance = ref_polynomial.eval(weight.ref_time());

			// Get fee amount from proof_size based on the ProofSize polynomial
			let proof_fee: Balance = proof_polynomial.eval(weight.proof_size());

			// Take the maximum instead of the sum to charge by the more scarce resource.
			ref_fee.max(proof_fee).saturating_mul(FEE_MULTIPLIER)
		}
	}

	/// Maps the Ref time component of `Weight` to a fee.
	pub struct RefTimeToFee;
	impl WeightToFeePolynomial for RefTimeToFee {
		type Balance = Balance;
		fn polynomial() -> WeightToFeeCoefficients<Self::Balance> {
			let numerator = MILLI_MYTH / 10;
			let denominator = 100 * Balance::from(ExtrinsicBaseWeight::get().ref_time());
			smallvec![WeightToFeeCoefficient {
				degree: 1,       // lineal function
				negative: false, // positive growth
				coeff_frac: Perbill::from_rational(numerator % denominator, denominator),
				coeff_integer: numerator / denominator,
			}]
		}
	}

	/// Maps the proof size component of `Weight` to a fee.
	pub struct ProofSizeToFee;
	impl WeightToFeePolynomial for ProofSizeToFee {
		type Balance = Balance;
		fn polynomial() -> WeightToFeeCoefficients<Self::Balance> {
			// Map 10kb proof to 10 MILLI_MYTH.
			let numerator = MILLI_MYTH * 10;
			let denominator = 10_000;

			smallvec![WeightToFeeCoefficient {
				degree: 1,       // lineal function
				negative: false, // positive growth
				coeff_frac: Perbill::from_rational(numerator % denominator, denominator),
				coeff_integer: numerator / denominator,
			}]
		}
	}

	pub fn base_relay_tx_fee() -> Balance {
		MILLI_DOT
	}

	pub fn default_fee_per_second() -> u128 {
		let base_weight = Balance::from(ExtrinsicBaseWeight::get().ref_time());
		let base_tx_per_second = (WEIGHT_REF_TIME_PER_SECOND as u128) / base_weight;
		base_tx_per_second * base_relay_tx_fee()
	}
}
/// Opaque types. These are used by the CLI to instantiate machinery that don't need to know
/// the specifics of the runtime. They can then be made to be agnostic over specific formats
/// of data like extrinsics, allowing for them to continue syncing the network through upgrades
/// to even the core data structures.
pub mod opaque {
	use super::*;
	use sp_runtime::traits::Hash as HashT;

	pub use sp_runtime::OpaqueExtrinsic as UncheckedExtrinsic;
	/// Opaque block header type.
	pub type Header = generic::Header<BlockNumber, BlakeTwo256>;
	/// Opaque block type.
	pub type Block = generic::Block<Header, UncheckedExtrinsic>;
	/// Opaque block identifier type.
	pub type BlockId = generic::BlockId<Block>;
	/// Opaque block hash type.
	pub type Hash = <BlakeTwo256 as HashT>::Output;
}

impl_opaque_keys! {
	pub struct SessionKeys {
		pub aura: Aura,
	}
}

mod async_backing_params {
	/// Maximum number of blocks simultaneously accepted by the Runtime, not yet included
	/// into the relay chain.
	pub(crate) const UNINCLUDED_SEGMENT_CAPACITY: u32 = 3;
	/// How many parachain blocks are processed by the relay chain per parent. Limits the
	/// number of blocks authored per slot.
	pub(crate) const BLOCK_PROCESSING_VELOCITY: u32 = 1;
	/// Relay chain slot duration, in milliseconds.
	pub(crate) const RELAY_CHAIN_SLOT_DURATION_MILLIS: u32 = 6000;
}
pub(crate) use async_backing_params::*;

pub type ConsensusHook = cumulus_pallet_aura_ext::FixedVelocityConsensusHook<
	Runtime,
	RELAY_CHAIN_SLOT_DURATION_MILLIS,
	BLOCK_PROCESSING_VELOCITY,
	UNINCLUDED_SEGMENT_CAPACITY,
>;

#[sp_version::runtime_version]
pub const VERSION: RuntimeVersion = RuntimeVersion {
	spec_name: alloc::borrow::Cow::Borrowed("mythos"),
	impl_name: alloc::borrow::Cow::Borrowed("mythos"),
	authoring_version: 1,
	spec_version: 1015,
	impl_version: 0,
	apis: apis::RUNTIME_API_VERSIONS,
	transaction_version: 1,
	system_version: 1,
};

pub const MICRO_MYTH: Balance = 1_000_000_000_000;
pub const MILLI_MYTH: Balance = 1_000 * MICRO_MYTH;
pub const MYTH: Balance = 1_000 * MILLI_MYTH;
// DOT has 10 decimal places
pub const MICRO_DOT: Balance = 10_000;
pub const MILLI_DOT: Balance = 1_000 * MICRO_DOT;

pub const EXISTENTIAL_DEPOSIT: Balance = 10 * MILLI_MYTH;

/// Calculate the storage deposit based on the number of storage items and the
/// combined byte size of those items.
pub const fn deposit(items: u32, bytes: u32) -> Balance {
	let per_item_deposit = MYTH / 5;
	let per_byte_deposit = MICRO_MYTH;
	items as Balance * per_item_deposit + (bytes as Balance) * per_byte_deposit
}

/// The version information used to identify this runtime when compiled natively.
#[cfg(feature = "std")]
pub fn native_version() -> NativeVersion {
	NativeVersion { runtime_version: VERSION, can_author_with: Default::default() }
}

// Create the runtime by composing the FRAME pallets that were previously configured.
#[frame_support::runtime]
mod runtime {
	#[runtime::runtime]
	#[runtime::derive(
		RuntimeCall,
		RuntimeEvent,
		RuntimeError,
		RuntimeOrigin,
		RuntimeFreezeReason,
		RuntimeHoldReason,
		RuntimeSlashReason,
		RuntimeLockId,
		RuntimeTask,
		RuntimeViewFunction
	)]
	pub struct Runtime;

	#[runtime::pallet_index(0)]
	pub type System = frame_system;
	#[runtime::pallet_index(1)]
	pub type ParachainSystem = cumulus_pallet_parachain_system;
	#[runtime::pallet_index(2)]
	pub type Timestamp = pallet_timestamp;
	#[runtime::pallet_index(3)]
	pub type ParachainInfo = parachain_info;
	#[runtime::pallet_index(4)]
	pub type WeightReclaim = cumulus_pallet_weight_reclaim;

	// Utility
	#[runtime::pallet_index(5)]
	pub type Multisig = pallet_multisig;
	#[runtime::pallet_index(6)]
	pub type Preimage = pallet_preimage;
	#[runtime::pallet_index(7)]
	pub type Scheduler = pallet_scheduler;
	#[runtime::pallet_index(8)]
	pub type Utility = pallet_utility; // was previously 4
	#[runtime::pallet_index(9)]
	pub type Identity = pallet_identity;

	// Monetary stuff.
	#[runtime::pallet_index(10)]
	pub type Balances = pallet_balances;
	#[runtime::pallet_index(11)]
	pub type TransactionPayment = pallet_transaction_payment;

	// NFTs
	#[runtime::pallet_index(12)]
	pub type Nfts = pallet_nfts;
	#[runtime::pallet_index(13)]
	pub type Marketplace = pallet_marketplace;
	#[runtime::pallet_index(14)]
	pub type Multibatching = pallet_multibatching;

	// Governance
	#[runtime::pallet_index(15)]
	pub type Sudo = pallet_sudo;
	#[runtime::pallet_index(16)]
	pub type Council = pallet_collective<Instance1>;
	#[runtime::pallet_index(17)]
	pub type Democracy = pallet_democracy;
	#[runtime::pallet_index(18)]
	pub type Treasury = pallet_treasury;

	// Collator support. The order of these 4 are important and shall not change.
	#[runtime::pallet_index(20)]
	pub type Authorship = pallet_authorship;
	#[runtime::pallet_index(21)]
	pub type CollatorStaking = pallet_collator_staking;
	#[runtime::pallet_index(22)]
	pub type Session = pallet_session;
	#[runtime::pallet_index(23)]
	pub type Aura = pallet_aura;
	#[runtime::pallet_index(24)]
	pub type AuraExt = cumulus_pallet_aura_ext;

	// XCM helpers.
	#[runtime::pallet_index(30)]
	pub type XcmpQueue = cumulus_pallet_xcmp_queue;
	#[runtime::pallet_index(31)]
	pub type PolkadotXcm = pallet_xcm;
	#[runtime::pallet_index(32)]
	pub type CumulusXcm = cumulus_pallet_xcm;
	#[runtime::pallet_index(33)]
	pub type MessageQueue = pallet_message_queue;

	// Other pallets.
	#[runtime::pallet_index(40)]
	pub type Proxy = pallet_proxy;
	#[runtime::pallet_index(41)]
	pub type Vesting = pallet_vesting;

	#[runtime::pallet_index(50)]
	pub type Escrow = pallet_escrow;
	#[runtime::pallet_index(51)]
	pub type MythProxy = pallet_myth_proxy;
	#[runtime::pallet_index(52)]
	pub type Dmarket = pallet_dmarket;
}

cumulus_pallet_parachain_system::register_validate_block! {
	Runtime = Runtime,
	BlockExecutor = cumulus_pallet_aura_ext::BlockExecutor::<Runtime, Executive>,
}
