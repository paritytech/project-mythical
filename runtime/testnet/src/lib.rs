#![cfg_attr(not(feature = "std"), no_std)]
// `construct_runtime!` does a lot of recursion and requires us to increase the limit to 256.
#![recursion_limit = "256"]

extern crate alloc; // Make the WASM binary available.
#[cfg(feature = "std")]
include!(concat!(env!("OUT_DIR"), "/wasm_binary.rs"));

mod weights;
pub mod xcm_config;

use core::marker::PhantomData;
pub use fee::WeightToFee;

use cumulus_pallet_parachain_system::RelayNumberMonotonicallyIncreases;
use cumulus_primitives_core::{AggregateMessageOrigin, AssetId, ParaId};

#[cfg(feature = "runtime-benchmarks")]
use pallet_treasury::ArgumentsFactory;
#[cfg(feature = "runtime-benchmarks")]
use sp_core::crypto::FromEntropy;

use sp_api::impl_runtime_apis;
use sp_core::{crypto::KeyTypeId, ConstBool, OpaqueMetadata};
use sp_runtime::{
	generic, impl_opaque_keys,
	traits::{BlakeTwo256, Block as BlockT, IdentityLookup, Verify},
	transaction_validity::{TransactionSource, TransactionValidity},
	ApplyExtrinsicResult, ExtrinsicInclusionMode,
};

use sp_std::prelude::*;
#[cfg(feature = "std")]
use sp_version::NativeVersion;
use sp_version::RuntimeVersion;

use frame_support::traits::{
	fungible,
	fungible::{Balanced, HoldConsideration},
	tokens::{PayFromAccount, UnityAssetBalanceConversion},
	AsEnsureOriginWithArg, InstanceFilter, LinearStoragePrice, OnUnbalanced, WithdrawReasons,
};
use frame_support::{
	construct_runtime, derive_impl,
	dispatch::DispatchClass,
	genesis_builder_helper::{build_state, get_preset},
	pallet_prelude::DispatchResult,
	parameter_types,
	traits::{ConstU32, ConstU64, ConstU8, EitherOfDiverse},
	weights::{ConstantMultiplier, Weight},
	PalletId,
};
use frame_system::{
	limits::{BlockLength, BlockWeights},
	EnsureRoot, EnsureSigned, EnsureWithSuccess,
};
use pallet_dmarket::{Item, TradeParams};
use pallet_nfts::PalletFeatures;
use parachains_common::message_queue::{NarrowOriginToSibling, ParaIdToSibling};
use parity_scale_codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use polkadot_primitives::Moment;
pub use runtime_common::{
	AccountId, Balance, BlockNumber, Hash, IncrementableU256, Nonce, Signature,
	AVERAGE_ON_INITIALIZE_RATIO, DAYS, HOURS, MAXIMUM_BLOCK_WEIGHT, MINUTES, NORMAL_DISPATCH_RATIO,
	SLOT_DURATION,
};
pub use sp_consensus_aura::sr25519::AuthorityId as AuraId;
use sp_runtime::traits::ConvertInto;
pub use sp_runtime::{MultiAddress, Perbill, Permill};
use xcm::{prelude::XcmVersion, VersionedLocation, VersionedXcm};
use xcm_config::XcmOriginToTransactDispatchOrigin;
use xcm_runtime_apis::dry_run::{CallDryRunEffects, Error as XcmDryRunApiError, XcmDryRunEffects};

#[cfg(any(feature = "std", test))]
pub use sp_runtime::BuildStorage;

// Polkadot imports
use polkadot_runtime_common::{BlockHashCount, SlowAdjustingFeeUpdate};

use weights::{BlockExecutionWeight, ExtrinsicBaseWeight, RocksDbWeight};

// XCM Imports
use crate::xcm_config::SelfReserve;
use runtime_common::AccountIdOf;

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

pub struct PrepareForMove;
impl frame_support::traits::OnRuntimeUpgrade for PrepareForMove {
	fn on_runtime_upgrade() -> Weight {
		// This is taken from https://hackmd.io/@bkchr/BkorHJMaA
		cumulus_pallet_parachain_system::LastHrmpMqcHeads::<Runtime>::kill();
		cumulus_pallet_parachain_system::LastDmqMqcHead::<Runtime>::kill();

		// This is taken from https://github.com/paseo-network/support/blob/main/docs/rococo_migration.md#2-migrate-state-and-history
		cumulus_pallet_parachain_system::LastRelayChainBlockNumber::<Runtime>::kill();

		// Weight negligible as the block will always fail to build
		Weight::zero()
	}
}

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

/// Implementation of `OnUnbalanced` that deals with the fees by combining tip and fee and burning
/// the fee.
pub struct DealWithFees<R>(PhantomData<R>);
impl<R> OnUnbalanced<fungible::Credit<R::AccountId, pallet_balances::Pallet<R>>> for DealWithFees<R>
where
	R: pallet_balances::Config + pallet_authorship::Config,
	AccountIdOf<R>: From<account::AccountId20> + Into<account::AccountId20>,
	<R as frame_system::Config>::RuntimeEvent: From<pallet_balances::Event<R>>,
{
	fn on_unbalanceds(
		mut fees_then_tips: impl Iterator<
			Item = fungible::Credit<R::AccountId, pallet_balances::Pallet<R>>,
		>,
	) {
		// We discard the fees, as they will get burned.
		let _ = fees_then_tips.next();

		// If there is a tip for the author we deliver it.
		if let Some(tips) = fees_then_tips.next() {
			if let Some(author) = <pallet_authorship::Pallet<R>>::author() {
				let _ = <pallet_balances::Pallet<R>>::resolve(&author, tips);
			}
		}
	}
}

pub mod fee {
	use super::{Balance, ExtrinsicBaseWeight, MILLI_MUSE, MILLI_ROC};
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
			let numerator = MILLI_MUSE / 10;
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
			// Map 10kb proof to 1/10 MILLI_MUSE.
			let numerator = MILLI_MUSE * 10;
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
		MILLI_ROC
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

#[sp_version::runtime_version]
pub const VERSION: RuntimeVersion = RuntimeVersion {
	spec_name: alloc::borrow::Cow::Borrowed("muse"),
	impl_name: alloc::borrow::Cow::Borrowed("muse"),
	authoring_version: 1,
	spec_version: 1027,
	impl_version: 0,
	apis: RUNTIME_API_VERSIONS,
	transaction_version: 1,
	system_version: 1,
};

pub const MICRO_MUSE: Balance = 1_000_000_000_000;
pub const MILLI_MUSE: Balance = 1_000 * MICRO_MUSE;
pub const MUSE: Balance = 1_000 * MILLI_MUSE;
//ROC has 12 decimal places
pub const MICRO_ROC: Balance = 1_000_000;
pub const MILLI_ROC: Balance = 1_000 * MICRO_ROC;

pub const EXISTENTIAL_DEPOSIT: Balance = 10 * MILLI_MUSE;

/// Calculate the storage deposit based on the number of storage items and the
/// combined byte size of those items.
pub const fn deposit(items: u32, bytes: u32) -> Balance {
	let per_item_deposit = MUSE / 5;
	let per_byte_deposit = MICRO_MUSE;
	items as Balance * per_item_deposit + (bytes as Balance) * per_byte_deposit
}

/// The version information used to identify this runtime when compiled natively.
#[cfg(feature = "std")]
pub fn native_version() -> NativeVersion {
	NativeVersion { runtime_version: VERSION, can_author_with: Default::default() }
}

/// Privileged origin that represents Root or more than two thirds of the Council.
pub type RootOrCouncilTwoThirdsMajority = EitherOfDiverse<
	EnsureRoot<AccountId>,
	pallet_collective::EnsureProportionMoreThan<AccountId, CouncilCollective, 2, 3>,
>;

parameter_types! {
	pub const Version: RuntimeVersion = VERSION;

	// This part is copied from Substrate's `bin/node/runtime/src/lib.rs`.
	//  The `RuntimeBlockLength` and `RuntimeBlockWeights` exist here because the
	// `DeletionWeightLimit` and `DeletionQueueDepth` depend on those to parameterize
	// the lazy contract deletion.
	pub RuntimeBlockLength: BlockLength =
		BlockLength::max_with_normal_ratio(5 * 1024 * 1024, NORMAL_DISPATCH_RATIO);
	pub RuntimeBlockWeights: BlockWeights = BlockWeights::builder()
		.base_block(BlockExecutionWeight::get())
		.for_class(DispatchClass::all(), |weights| {
			weights.base_extrinsic = ExtrinsicBaseWeight::get();
		})
		.for_class(DispatchClass::Normal, |weights| {
			weights.max_total = Some(NORMAL_DISPATCH_RATIO * MAXIMUM_BLOCK_WEIGHT);
		})
		.for_class(DispatchClass::Operational, |weights| {
			weights.max_total = Some(MAXIMUM_BLOCK_WEIGHT);
			// Operational transactions have some extra reserved space, so that they
			// are included even if block reached `MAXIMUM_BLOCK_WEIGHT`.
			weights.reserved = Some(
				MAXIMUM_BLOCK_WEIGHT - NORMAL_DISPATCH_RATIO * MAXIMUM_BLOCK_WEIGHT
			);
		})
		.avg_block_initialization(AVERAGE_ON_INITIALIZE_RATIO)
		.build_or_panic();
	pub MaxCollectivesProposalWeight: Weight = Perbill::from_percent(50) * RuntimeBlockWeights::get().max_block;
	/// SS58 prefix of the parachain. Used for address formatting.
	pub const SS58Prefix: u16 = 29972;
}

// Configure FRAME pallets to include in runtime.

#[derive_impl(frame_system::config_preludes::ParaChainDefaultConfig as frame_system::DefaultConfig)]
impl frame_system::Config for Runtime {
	/// Block & extrinsics weights: base values and limits.
	type BlockWeights = RuntimeBlockWeights;
	/// The maximum length of a block (in bytes).
	type BlockLength = RuntimeBlockLength;
	/// This stores the number of previous transactions associated with a sender account.
	type Nonce = Nonce;
	/// The type for hashing blocks and tries.
	type Hash = Hash;
	/// The identifier used to distinguish between accounts.
	type AccountId = AccountId;
	/// The lookup mechanism to get account ID from whatever is passed in dispatchers.
	type Lookup = sp_runtime::traits::IdentityLookup<AccountId>;
	/// The Block type used by the runtime. This is used by `construct_runtime` to retrieve the
	/// extrinsics or other block specific data as needed.
	type Block = Block;
	/// Maximum number of block number to block hash mappings to keep (oldest pruned first).
	type BlockHashCount = BlockHashCount;
	/// The weight of database operations that the runtime can invoke.
	type DbWeight = RocksDbWeight;
	/// Runtime version.
	type Version = Version;
	/// The data to be stored in an account.
	type AccountData = pallet_balances::AccountData<Balance>;
	/// This is used as an identifier of the chain. 42 is the generic substrate prefix.
	type SS58Prefix = SS58Prefix;
	/// The action to take on a Runtime Upgrade
	type OnSetCode = cumulus_pallet_parachain_system::ParachainSetCode<Self>;
	/// The maximum number of consumers allowed on a single account.
	type MaxConsumers = frame_support::traits::ConstU32<16>;
	type SystemWeightInfo = weights::frame_system::WeightInfo<Runtime>;
}

impl pallet_timestamp::Config for Runtime {
	/// A timestamp: milliseconds since the unix epoch.
	type Moment = u64;
	type OnTimestampSet = Aura;
	type MinimumPeriod = ConstU64<0>;
	type WeightInfo = weights::pallet_timestamp::WeightInfo<Runtime>;
}

parameter_types! {
	pub const UncleGenerations: u32 = 0;
}

impl pallet_authorship::Config for Runtime {
	type FindAuthor = pallet_session::FindAccountFromAuthorIndex<Self, Aura>;
	type EventHandler = (CollatorStaking,);
}

parameter_types! {
	pub const ExistentialDeposit: Balance = EXISTENTIAL_DEPOSIT;
}

impl pallet_balances::Config for Runtime {
	/// The ubiquitous event type.
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = weights::pallet_balances::WeightInfo<Runtime>;
	/// The type for recording an account's balance.
	type Balance = Balance;
	type DustRemoval = ();
	type ExistentialDeposit = ExistentialDeposit;
	type AccountStore = System;
	type ReserveIdentifier = [u8; 8];
	type RuntimeHoldReason = RuntimeHoldReason;
	type FreezeIdentifier = RuntimeFreezeReason;
	type MaxLocks = ConstU32<50>;
	type MaxReserves = ConstU32<50>;
	type MaxFreezes = ConstU32<50>;
	type RuntimeFreezeReason = RuntimeFreezeReason;
	type DoneSlashHandler = ();
}

parameter_types! {
	pub const DOMAIN: [u8;8] = *b"MUSE_NET";
}

impl pallet_multibatching::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type RuntimeCall = RuntimeCall;
	type Signature = Signature;
	type Signer = <Signature as Verify>::Signer;
	type MaxCalls = ConstU32<128>;
	type Domain = DOMAIN;
	type WeightInfo = weights::pallet_multibatching::WeightInfo<Runtime>;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = ();
}

parameter_types! {
	pub const TransactionByteFee: Balance = fee::FEE_MULTIPLIER * 100 * MICRO_MUSE;
}

impl pallet_transaction_payment::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type OnChargeTransaction =
		pallet_transaction_payment::FungibleAdapter<Balances, DealWithFees<Runtime>>;
	type OperationalFeeMultiplier = ConstU8<5>;
	type WeightToFee = WeightToFee;
	type LengthToFee = ConstantMultiplier<Balance, TransactionByteFee>;
	type FeeMultiplierUpdate = SlowAdjustingFeeUpdate<Self>;
	type WeightInfo = weights::pallet_transaction_payment::WeightInfo<Runtime>;
}

impl pallet_utility::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type RuntimeCall = RuntimeCall;
	type PalletsOrigin = OriginCaller;
	type WeightInfo = weights::pallet_utility::WeightInfo<Runtime>;
}

parameter_types! {
	pub const ReservedXcmpWeight: Weight = MAXIMUM_BLOCK_WEIGHT.saturating_div(4);
	pub const ReservedDmpWeight: Weight = MAXIMUM_BLOCK_WEIGHT.saturating_div(4);
	pub const RelayOrigin: AggregateMessageOrigin = AggregateMessageOrigin::Parent;
}

/// Maximum number of blocks simultaneously accepted by the Runtime, not yet included
/// into the relay chain.
const UNINCLUDED_SEGMENT_CAPACITY: u32 = 3;
/// How many parachain blocks are processed by the relay chain per parent. Limits the
/// number of blocks authored per slot.
const BLOCK_PROCESSING_VELOCITY: u32 = 1;
/// Relay chain slot duration, in milliseconds.
const RELAY_CHAIN_SLOT_DURATION_MILLIS: u32 = 6000;

impl cumulus_pallet_parachain_system::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type OnSystemEvent = ();
	type SelfParaId = parachain_info::Pallet<Runtime>;
	type OutboundXcmpMessageSource = XcmpQueue;
	type DmpQueue = frame_support::traits::EnqueueWithOrigin<MessageQueue, RelayOrigin>;
	type ReservedDmpWeight = ReservedDmpWeight;
	type XcmpMessageHandler = XcmpQueue;
	type ReservedXcmpWeight = ReservedXcmpWeight;
	type CheckAssociatedRelayNumber = RelayNumberMonotonicallyIncreases;
	type ConsensusHook = ConsensusHook;
	type WeightInfo = weights::cumulus_pallet_parachain_system::WeightInfo<Runtime>;
	type SelectCore = cumulus_pallet_parachain_system::DefaultCoreSelector<Runtime>;
}

type ConsensusHook = cumulus_pallet_aura_ext::FixedVelocityConsensusHook<
	Runtime,
	RELAY_CHAIN_SLOT_DURATION_MILLIS,
	BLOCK_PROCESSING_VELOCITY,
	UNINCLUDED_SEGMENT_CAPACITY,
>;

impl parachain_info::Config for Runtime {}

impl cumulus_pallet_aura_ext::Config for Runtime {}

parameter_types! {
	pub MessageQueueServiceWeight: Weight = Perbill::from_percent(35) * RuntimeBlockWeights::get().max_block;
}

impl pallet_message_queue::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = weights::pallet_message_queue::WeightInfo<Runtime>;
	#[cfg(feature = "runtime-benchmarks")]
	type MessageProcessor = pallet_message_queue::mock_helpers::NoopMessageProcessor<
		cumulus_primitives_core::AggregateMessageOrigin,
	>;
	#[cfg(not(feature = "runtime-benchmarks"))]
	type MessageProcessor = xcm_builder::ProcessXcmMessage<
		AggregateMessageOrigin,
		xcm_executor::XcmExecutor<xcm_config::XcmConfig>,
		RuntimeCall,
	>;
	type Size = u32;
	// The XCMP queue pallet is only ever able to handle the `Sibling(ParaId)` origin:
	type QueueChangeHandler = NarrowOriginToSibling<XcmpQueue>;
	type QueuePausedQuery = NarrowOriginToSibling<XcmpQueue>;
	type HeapSize = sp_core::ConstU32<{ 64 * 1024 }>;
	type MaxStale = sp_core::ConstU32<8>;
	type ServiceWeight = MessageQueueServiceWeight;
	type IdleMaxServiceWeight = MessageQueueServiceWeight;
}

parameter_types! {
	/// The asset ID for the asset that we use to pay for message delivery fees.
	pub FeeAssetId: AssetId = AssetId(SelfReserve::get());
	/// The base fee for the message delivery fees.
	pub const BaseDeliveryFee: Balance = 300_000_000;
	/// The fee per byte
	pub const ByteFee: Balance = 1_000_000;
}

pub type PriceForSiblingParachainDelivery = polkadot_runtime_common::xcm_sender::ExponentialPrice<
	FeeAssetId,
	BaseDeliveryFee,
	ByteFee,
	XcmpQueue,
>;

impl cumulus_pallet_xcmp_queue::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type ChannelInfo = ParachainSystem;
	type VersionWrapper = PolkadotXcm;
	// Enqueue XCMP messages from siblings for later processing.
	type XcmpQueue = frame_support::traits::TransformOrigin<
		MessageQueue,
		AggregateMessageOrigin,
		ParaId,
		ParaIdToSibling,
	>;
	type MaxInboundSuspended = sp_core::ConstU32<1_000>;
	type ControllerOrigin = EnsureRoot<AccountId>;
	type ControllerOriginConverter = XcmOriginToTransactDispatchOrigin;
	type PriceForSiblingDelivery = PriceForSiblingParachainDelivery;
	type WeightInfo = weights::cumulus_pallet_xcmp_queue::WeightInfo<Runtime>;
	type MaxActiveOutboundChannels = ConstU32<128>;
	type MaxPageSize = ConstU32<{ 103 * 1024 }>;
}

impl cumulus_pallet_weight_reclaim::Config for Runtime {
	type WeightInfo = weights::cumulus_pallet_weight_reclaim::WeightInfo<Runtime>;
}

parameter_types! {
	pub const Period: u32 = 25;
	pub const Offset: u32 = 0;
}

impl pallet_session::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type ValidatorId = <Self as frame_system::Config>::AccountId;
	// we don't have stash and controller, thus we don't need the convert as well.
	type ValidatorIdOf = pallet_collator_staking::IdentityCollator;
	type ShouldEndSession = pallet_session::PeriodicSessions<Period, Offset>;
	type NextSessionRotation = pallet_session::PeriodicSessions<Period, Offset>;
	type SessionManager = CollatorStaking;
	// Essentially just Aura, but lets be pedantic.
	type SessionHandler = <SessionKeys as sp_runtime::traits::OpaqueKeys>::KeyTypeIdProviders;
	type Keys = SessionKeys;
	type DisablingStrategy = pallet_session::disabling::UpToLimitWithReEnablingDisablingStrategy;
	type WeightInfo = weights::pallet_session::WeightInfo<Runtime>;
}

impl pallet_sudo::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type RuntimeCall = RuntimeCall;
	type WeightInfo = weights::pallet_sudo::WeightInfo<Runtime>;
}

parameter_types! {
	// One storage item; key size is 32; value is size 4+4+16+32 bytes = 56 bytes.
	pub const DepositBase: Balance = deposit(1, 88);
	// Additional storage item size of 32 bytes.
	pub const DepositFactor: Balance = deposit(0, 32);
}

impl pallet_multisig::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type RuntimeCall = RuntimeCall;
	type Currency = Balances;
	type DepositBase = DepositBase;
	type DepositFactor = DepositFactor;
	type MaxSignatories = ConstU32<100>;
	type WeightInfo = weights::pallet_multisig::WeightInfo<Runtime>;
	type BlockNumberProvider = frame_system::Pallet<Runtime>;
}

impl pallet_aura::Config for Runtime {
	type AuthorityId = AuraId;
	type MaxAuthorities = ConstU32<100_000>;
	type DisabledValidators = ();
	type AllowMultipleBlocksPerSlot = ConstBool<true>;
	type SlotDuration = ConstU64<SLOT_DURATION>;
}

parameter_types! {
	pub const PotId: PalletId = PalletId(*b"PotStake");
	pub const ExtraRewardPotId: PalletId = PalletId(*b"ExtraPot");
	pub const MaxCandidates: u32 = 15;
	pub const MinEligibleCollators: u32 = 2;
	pub const MaxInvulnerables: u32 = 4;
	pub const MaxStakers: u32 = 200_000;
	pub const KickThreshold: u32 = 2 * Period::get();
	pub const BondUnlockDelay: BlockNumber = 5 * MINUTES;
	pub const StakeUnlockDelay: BlockNumber = 2 * MINUTES;
	pub const AutoCompoundingThreshold: Balance = 50 * MUSE;
	/// Rewards are claimable for up to a year.
	/// Pending to claim rewards past a year will be lost.
	pub const MaxRewardSessions: u32 = 365;
	pub const MaxStakedCandidates: u32 = 3;
}

impl pallet_collator_staking::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type RuntimeFreezeReason = RuntimeFreezeReason;
	type UpdateOrigin = RootOrCouncilTwoThirdsMajority;
	type PotId = PotId;
	type ExtraRewardPotId = ExtraRewardPotId;
	type ExtraRewardReceiver = TreasuryAccount;
	type MaxCandidates = MaxCandidates;
	type MinEligibleCollators = MinEligibleCollators;
	type MaxInvulnerables = MaxInvulnerables;
	// should be a multiple of session or things will get inconsistent
	type KickThreshold = KickThreshold;
	type CollatorId = <Self as frame_system::Config>::AccountId;
	type CollatorIdOf = pallet_collator_staking::IdentityCollator;
	type CollatorRegistration = Session;
	type MaxStakedCandidates = MaxStakedCandidates;
	type MaxStakers = MaxStakers;
	type BondUnlockDelay = BondUnlockDelay;
	type StakeUnlockDelay = StakeUnlockDelay;
	type RestakeUnlockDelay = Period;
	type MaxRewardSessions = MaxRewardSessions;
	type AutoCompoundingThreshold = AutoCompoundingThreshold;
	type WeightInfo = weights::pallet_collator_staking::WeightInfo<Runtime>;
}

// Project specific pallets.

parameter_types! {
	pub NftsPalletFeatures: PalletFeatures = PalletFeatures::all_enabled();
	pub const NftsMaxDeadlineDuration: BlockNumber = 12 * 30 * DAYS;
	pub const NftsCollectionDeposit: Balance = 0;
	pub const NftsMetadataDepositBase: Balance = 0;
	pub const NftsAttributeDepositBase: Balance = 0;
	pub const NftsDepositPerByte: Balance = 0;
}

#[cfg(not(feature = "runtime-benchmarks"))]
parameter_types! {
	pub const NftsItemDeposit: Balance = 0;
}

#[cfg(feature = "runtime-benchmarks")]
parameter_types! {
	pub const NftsItemDeposit: Balance = EXISTENTIAL_DEPOSIT;
}

pub type CollectionId = IncrementableU256;

impl pallet_nfts::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type CollectionId = CollectionId;
	type Currency = Balances;
	type CreateOrigin = AsEnsureOriginWithArg<frame_system::EnsureSigned<AccountId>>;
	type ForceOrigin = RootOrCouncilTwoThirdsMajority;
	type Locker = ();
	type CollectionDeposit = NftsCollectionDeposit;
	type ItemDeposit = NftsItemDeposit;
	type MetadataDepositBase = NftsMetadataDepositBase;
	type AttributeDepositBase = NftsAttributeDepositBase;
	type DepositPerByte = NftsDepositPerByte;
	type StringLimit = ConstU32<256>;
	type KeyLimit = ConstU32<64>;
	type ValueLimit = ConstU32<256>;
	type ApprovalsLimit = ConstU32<20>;
	type ItemAttributesApprovalsLimit = ConstU32<30>;
	type MaxTips = ConstU32<10>;
	type MaxDeadlineDuration = NftsMaxDeadlineDuration;
	type MaxAttributesPerCall = ConstU32<10>;
	type Features = NftsPalletFeatures;
	type OffchainSignature = Signature;
	type OffchainPublic = <Signature as Verify>::Signer;
	type WeightInfo = weights::pallet_nfts::WeightInfo<Runtime>;
	#[cfg(feature = "runtime-benchmarks")]
	type Helper = ();
}

pub struct EscrowImpl;

impl pallet_marketplace::Escrow<AccountId, Balance, AccountId> for EscrowImpl {
	fn make_deposit(
		depositor: &AccountId,
		destination: &AccountId,
		value: Balance,
		escrow_agent: &AccountId,
	) -> DispatchResult {
		Escrow::make_deposit(depositor, destination, value, escrow_agent)
	}
}

impl pallet_marketplace::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type RuntimeCall = RuntimeCall;
	type Currency = Balances;
	type Escrow = EscrowImpl;
	type RuntimeHoldReason = RuntimeHoldReason;
	type MinOrderDuration = ConstU64<10>;
	type NonceStringLimit = ConstU32<50>;
	type Signature = Signature;
	type Signer = <Signature as Verify>::Signer;
	type WeightInfo = weights::pallet_marketplace::WeightInfo<Runtime>;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = ();
}

impl pallet_dmarket::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type RuntimeCall = RuntimeCall;
	type Currency = Balances;
	type Signature = Signature;
	type Signer = <Signature as Verify>::Signer;
	type Domain = DOMAIN;
	type WeightInfo = weights::pallet_dmarket::WeightInfo<Runtime>;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = ();
}

parameter_types! {
	pub const ProxyDepositBase: Balance = deposit(1, 8);
	pub const ProxyDepositFactor: Balance = deposit(0, 33);
	pub const AnnouncementDepositBase: Balance = deposit(1, 8);
	pub const AnnouncementDepositFactor: Balance = deposit(0, 66);
	pub const MaxPending: u16 = 32;
	pub const MaxProxies: u16 = 32;
}

#[derive(
	Copy,
	Clone,
	Eq,
	PartialEq,
	Ord,
	PartialOrd,
	Encode,
	Decode,
	DecodeWithMemTracking,
	MaxEncodedLen,
	scale_info::TypeInfo,
	Debug,
)]
pub enum ProxyType {
	/// All calls can be proxied. This is the trivial/most permissive filter.
	Any,
	/// Only extrinsics that do not transfer funds.
	NonTransfer,
	/// Allow to veto an announced proxy call.
	CancelProxy,
	/// Allow extrinsic related to Balances.
	Balances,
	/// Does not allow to create or remove proxies.
	RestrictProxyManagement,
	/// A proxy type dedicated to operations related to staking.
	Staking,
}

impl Default for ProxyType {
	fn default() -> Self {
		Self::Any
	}
}

impl InstanceFilter<RuntimeCall> for ProxyType {
	fn filter(&self, call: &RuntimeCall) -> bool {
		match self {
			ProxyType::Any => true,
			ProxyType::NonTransfer => {
				!matches!(call, RuntimeCall::Balances(..) | RuntimeCall::Escrow(..))
			},
			ProxyType::CancelProxy => {
				matches!(call, RuntimeCall::Proxy(pallet_proxy::Call::reject_announcement { .. }))
			},
			ProxyType::Balances => matches!(call, RuntimeCall::Balances(..)),
			ProxyType::RestrictProxyManagement => !matches!(
				call,
				RuntimeCall::Proxy(pallet_proxy::Call::add_proxy { .. })
					| RuntimeCall::Proxy(pallet_proxy::Call::create_pure { .. })
					| RuntimeCall::Proxy(pallet_proxy::Call::kill_pure { .. })
					| RuntimeCall::Proxy(pallet_proxy::Call::remove_proxies { .. })
					| RuntimeCall::Proxy(pallet_proxy::Call::remove_proxy { .. })
					| RuntimeCall::MythProxy(pallet_myth_proxy::Call::add_proxy { .. })
					| RuntimeCall::MythProxy(
						pallet_myth_proxy::Call::remove_sponsored_proxy { .. }
					) | RuntimeCall::MythProxy(pallet_myth_proxy::Call::remove_proxy { .. })
					| RuntimeCall::MythProxy(
						pallet_myth_proxy::Call::register_sponsor_agent { .. }
					) | RuntimeCall::MythProxy(pallet_myth_proxy::Call::revoke_sponsor_agent { .. })
			),
			ProxyType::Staking => matches!(call, RuntimeCall::CollatorStaking { .. }),
		}
	}
	fn is_superset(&self, o: &Self) -> bool {
		match (self, o) {
			(x, y) if x == y => true,
			(ProxyType::Any, _) => true,
			(_, ProxyType::Any) => false,
			_ => false,
		}
	}
}

impl pallet_proxy::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type RuntimeCall = RuntimeCall;
	type Currency = Balances;
	type ProxyType = ProxyType;
	type ProxyDepositBase = ProxyDepositBase;
	type ProxyDepositFactor = ProxyDepositFactor;
	type MaxProxies = MaxProxies;
	type WeightInfo = weights::pallet_proxy::WeightInfo<Runtime>;
	type MaxPending = MaxPending;
	type CallHasher = BlakeTwo256;
	type AnnouncementDepositBase = AnnouncementDepositBase;
	type AnnouncementDepositFactor = AnnouncementDepositFactor;
	type BlockNumberProvider = frame_system::Pallet<Runtime>;
}

impl pallet_escrow::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type Balance = Balance;
	type MinDeposit = ExistentialDeposit;
	type RuntimeHoldReason = RuntimeHoldReason;
	type WeightInfo = weights::pallet_escrow::WeightInfo<Runtime>;
}

impl pallet_myth_proxy::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type RuntimeCall = RuntimeCall;
	type ProxyType = ProxyType;
	type MaxProxies = MaxProxies;
	type ProxyDeposit = ProxyDepositBase;
	type Currency = Balances;
	type RuntimeHoldReason = RuntimeHoldReason;
	type WeightInfo = weights::pallet_myth_proxy::WeightInfo<Runtime>;
}

parameter_types! {
	pub const MinVestedTransfer: Balance = 100 * MILLI_MUSE;
	pub UnvestedFundsAllowedWithdrawReasons: WithdrawReasons =
		WithdrawReasons::except(WithdrawReasons::TRANSFER | WithdrawReasons::RESERVE);
}

impl pallet_vesting::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type BlockNumberToBalance = ConvertInto;
	type MinVestedTransfer = MinVestedTransfer;
	type WeightInfo = weights::pallet_vesting::WeightInfo<Runtime>;
	type UnvestedFundsAllowedWithdrawReasons = UnvestedFundsAllowedWithdrawReasons;
	type BlockNumberProvider = System;
	const MAX_VESTING_SCHEDULES: u32 = 28;
}

parameter_types! {
	pub const CouncilMotionDuration: BlockNumber = 10 * MINUTES;
	pub const CouncilMaxProposals: u32 = 100;
	pub const CouncilMaxMembers: u32 = 100;
}

type CouncilCollective = pallet_collective::Instance1;
impl pallet_collective::Config<CouncilCollective> for Runtime {
	type RuntimeOrigin = RuntimeOrigin;
	type Proposal = RuntimeCall;
	type RuntimeEvent = RuntimeEvent;
	type MotionDuration = CouncilMotionDuration;
	type MaxProposals = CouncilMaxProposals;
	type MaxMembers = CouncilMaxMembers;
	type DefaultVote = pallet_collective::PrimeDefaultVote;
	type WeightInfo = weights::pallet_collective::WeightInfo<Runtime>;
	type SetMembersOrigin = RootOrCouncilTwoThirdsMajority;
	type MaxProposalWeight = MaxCollectivesProposalWeight;
	type DisapproveOrigin = EnsureRoot<Self::AccountId>;
	type KillOrigin = EnsureRoot<Self::AccountId>;
	type Consideration = ();
}

parameter_types! {
	pub MaximumSchedulerWeight: Weight = Perbill::from_percent(80) *
	RuntimeBlockWeights::get().max_block;
	pub const NoPreimagePostponement: Option<u32> = Some(10);
}

impl pallet_scheduler::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type RuntimeOrigin = RuntimeOrigin;
	type PalletsOrigin = OriginCaller;
	type RuntimeCall = RuntimeCall;
	type MaximumWeight = MaximumSchedulerWeight;
	type ScheduleOrigin = RootOrCouncilTwoThirdsMajority;
	type OriginPrivilegeCmp = frame_support::traits::EqualPrivilegeOnly;
	#[cfg(feature = "runtime-benchmarks")]
	type MaxScheduledPerBlock = ConstU32<512>;
	#[cfg(not(feature = "runtime-benchmarks"))]
	type MaxScheduledPerBlock = ConstU32<50>;
	type WeightInfo = weights::pallet_scheduler::WeightInfo<Runtime>;
	type Preimages = Preimage;
	type BlockNumberProvider = frame_system::Pallet<Runtime>;
}

parameter_types! {
	pub const PreimageBaseDeposit: Balance = deposit(2, 64);
	pub const PreimageByteDeposit: Balance = deposit(0, 1);
	pub const PreimageHoldReason: RuntimeHoldReason = RuntimeHoldReason::Preimage(pallet_preimage::HoldReason::Preimage);
}

impl pallet_preimage::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = weights::pallet_preimage::WeightInfo<Runtime>;
	type Currency = Balances;
	type ManagerOrigin = RootOrCouncilTwoThirdsMajority;
	type Consideration = HoldConsideration<
		AccountId,
		Balances,
		PreimageHoldReason,
		LinearStoragePrice<PreimageBaseDeposit, PreimageByteDeposit, Balance>,
	>;
}

parameter_types! {
	pub const LaunchPeriod: BlockNumber = HOURS;
	pub const VotingPeriod: BlockNumber = 2 * DAYS;
	pub const FastTrackVotingPeriod: BlockNumber = DAYS;
	pub const MinimumDeposit: Balance = 10 * MILLI_MUSE;
	pub const EnactmentPeriod: BlockNumber = 3 * DAYS;
	pub const CooloffPeriod: BlockNumber = 2 * DAYS;
	pub const MaxProposals: u32 = 100;
}

impl pallet_democracy::Config for Runtime {
	type WeightInfo = weights::pallet_democracy::WeightInfo<Runtime>;
	type RuntimeEvent = RuntimeEvent;
	type Scheduler = Scheduler;
	type Preimages = Preimage;
	type Currency = Balances;
	type EnactmentPeriod = EnactmentPeriod;
	type LaunchPeriod = LaunchPeriod;
	type VotingPeriod = VotingPeriod;
	type VoteLockingPeriod = EnactmentPeriod;
	type MinimumDeposit = MinimumDeposit;
	type InstantAllowed = ConstBool<true>;
	type FastTrackVotingPeriod = FastTrackVotingPeriod;
	type CooloffPeriod = CooloffPeriod;
	type MaxVotes = ConstU32<100>;
	type MaxProposals = MaxProposals;
	type MaxDeposits = ConstU32<100>;
	type MaxBlacklisted = ConstU32<100>;
	type ExternalOrigin = EitherOfDiverse<
		EnsureRoot<AccountId>,
		pallet_collective::EnsureProportionAtLeast<AccountId, CouncilCollective, 1, 2>,
	>;
	type ExternalMajorityOrigin = EitherOfDiverse<
		EnsureRoot<AccountId>,
		pallet_collective::EnsureProportionAtLeast<AccountId, CouncilCollective, 3, 4>,
	>;
	type ExternalDefaultOrigin = EitherOfDiverse<
		EnsureRoot<AccountId>,
		pallet_collective::EnsureProportionAtLeast<AccountId, CouncilCollective, 1, 1>,
	>;
	type SubmitOrigin = EnsureSigned<AccountId>;
	type FastTrackOrigin = RootOrCouncilTwoThirdsMajority;
	type InstantOrigin = EitherOfDiverse<
		EnsureRoot<AccountId>,
		pallet_collective::EnsureProportionAtLeast<AccountId, CouncilCollective, 1, 1>,
	>;
	type CancellationOrigin = RootOrCouncilTwoThirdsMajority;
	type BlacklistOrigin = EnsureRoot<AccountId>;
	type CancelProposalOrigin = EitherOfDiverse<
		EnsureRoot<AccountId>,
		pallet_collective::EnsureProportionAtLeast<AccountId, CouncilCollective, 1, 1>,
	>;
	type VetoOrigin = pallet_collective::EnsureMember<AccountId, CouncilCollective>;
	type PalletsOrigin = OriginCaller;
	type Slash = Treasury;
}

parameter_types! {
	pub TreasuryAccount: AccountId = Treasury::account_id();
	pub const SpendPeriod: BlockNumber = 5 * MINUTES;
	pub const TreasuryPalletId: PalletId = PalletId(*b"py/trsry");
	pub const MaximumReasonLength: u32 = 300;
	pub const MaxApprovals: u32 = 100;
	pub const MaxBalance: Balance = Balance::MAX;
	pub const SpendPayoutPeriod: BlockNumber = 7 * DAYS;
}

pub struct TreasuryBenchmarkHelper<T>(PhantomData<T>);

#[cfg(feature = "runtime-benchmarks")]
impl<T> ArgumentsFactory<(), AccountId> for TreasuryBenchmarkHelper<T>
where
	T: fungible::Mutate<AccountId> + fungible::Inspect<AccountId>,
{
	fn create_asset_kind(_seed: u32) {
		// no-op
	}
	fn create_beneficiary(seed: [u8; 32]) -> AccountId {
		let account = AccountId::from_entropy(&mut seed.as_slice()).unwrap();
		<T as fungible::Mutate<_>>::mint_into(
			&account,
			<T as fungible::Inspect<_>>::minimum_balance(),
		)
		.unwrap();
		account
	}
}

impl pallet_treasury::Config for Runtime {
	type Currency = Balances;
	type RejectOrigin = RootOrCouncilTwoThirdsMajority;
	type RuntimeEvent = RuntimeEvent;
	type SpendPeriod = SpendPeriod;
	type Burn = ();
	type PalletId = TreasuryPalletId;
	type BurnDestination = ();
	type WeightInfo = weights::pallet_treasury::WeightInfo<Runtime>;
	type SpendFunds = Bounties;
	type MaxApprovals = MaxApprovals;
	type SpendOrigin = EnsureWithSuccess<RootOrCouncilTwoThirdsMajority, AccountId, MaxBalance>;
	type AssetKind = ();
	type Beneficiary = AccountId;
	type BeneficiaryLookup = IdentityLookup<Self::Beneficiary>;
	type Paymaster = PayFromAccount<Balances, TreasuryAccount>;
	type BalanceConverter = UnityAssetBalanceConversion;
	type PayoutPeriod = SpendPayoutPeriod;
	type BlockNumberProvider = frame_system::Pallet<Runtime>;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = TreasuryBenchmarkHelper<Balances>;
}

parameter_types! {
	pub const BountyDepositBase: Balance = 10 * MUSE;
	pub const BountyDepositPayoutDelay: BlockNumber = 0;
	pub const BountyUpdatePeriod: BlockNumber = 90 * MINUTES;
	pub const CuratorDepositMultiplier: Permill = Permill::from_percent(50);
	pub const CuratorDepositMin: Balance = 100 * MUSE;
	pub const CuratorDepositMax: Balance = 2000 * MUSE;
	pub const BountyValueMinimum: Balance = 100 * MUSE;
	pub const DataDepositPerByte: Balance = 100 * MILLI_MUSE;
}

impl pallet_bounties::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type BountyDepositBase = BountyDepositBase;
	type BountyDepositPayoutDelay = BountyDepositPayoutDelay;
	type BountyUpdatePeriod = BountyUpdatePeriod;
	type CuratorDepositMultiplier = CuratorDepositMultiplier;
	type CuratorDepositMin = CuratorDepositMin;
	type CuratorDepositMax = CuratorDepositMax;
	type BountyValueMinimum = BountyValueMinimum;
	type DataDepositPerByte = DataDepositPerByte;
	type MaximumReasonLength = MaximumReasonLength;
	type WeightInfo = weights::pallet_bounties::WeightInfo<Runtime>;
	type ChildBountyManager = ();
	type OnSlash = Treasury;
}

parameter_types! {
	//   27 | Min encoded size of `Registration`
	// - 10 | Min encoded size of `IdentityInfo`
	// -----|
	//   17 | Min size without `IdentityInfo` (accounted for in byte deposit)
	pub const BasicDeposit: Balance = deposit(10, 170);
	pub const ByteDeposit: Balance = deposit(0, 10);
	pub const UsernameDeposit: Balance = deposit(0, 320);
	pub const SubAccountDeposit: Balance = deposit(10, 530);
	pub const MaxSubAccounts: u32 = 100;
	pub const MaxRegistrars: u32 = 20;
}

impl pallet_identity::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type BasicDeposit = BasicDeposit;
	type ByteDeposit = ByteDeposit;
	type UsernameDeposit = UsernameDeposit;
	type SubAccountDeposit = SubAccountDeposit;
	type MaxSubAccounts = MaxSubAccounts;
	type IdentityInformation = runtime_common::IdentityInfo;
	type MaxRegistrars = MaxRegistrars;
	type Slashed = Treasury;
	type ForceOrigin = RootOrCouncilTwoThirdsMajority;
	type RegistrarOrigin = RootOrCouncilTwoThirdsMajority;
	type OffchainSignature = Signature;
	type SigningPublicKey = <Signature as Verify>::Signer;
	type UsernameAuthorityOrigin = RootOrCouncilTwoThirdsMajority;
	type PendingUsernameExpiration = ConstU32<{ 7 * MINUTES }>;
	type UsernameGracePeriod = ConstU32<{ 7 * MINUTES }>;
	type MaxSuffixLength = ConstU32<7>;
	type MaxUsernameLength = ConstU32<32>;
	type WeightInfo = ();
}

// Create the runtime by composing the FRAME pallets that were previously configured.
construct_runtime!(
	pub struct Runtime {
		// System support stuff.
		System: frame_system = 0,
		ParachainSystem: cumulus_pallet_parachain_system = 1,
		Timestamp: pallet_timestamp = 2,
		ParachainInfo: parachain_info = 3,
		WeightReclaim: cumulus_pallet_weight_reclaim = 4,

		// Utility
		Multisig: pallet_multisig = 5,
		Preimage: pallet_preimage = 6,
		Scheduler: pallet_scheduler = 7,
		Utility: pallet_utility = 8,  // was previously 4
		Identity: pallet_identity = 9,

		// Monetary stuff.
		Balances: pallet_balances = 10,
		TransactionPayment: pallet_transaction_payment = 11,

		// NFTs
		Nfts: pallet_nfts = 12,
		Marketplace: pallet_marketplace = 13,
		Multibatching: pallet_multibatching = 14,

		// Governance
		Sudo: pallet_sudo = 15,
		Council: pallet_collective::<Instance1> = 16,
		Democracy: pallet_democracy = 17,
		Treasury: pallet_treasury = 18,
		Bounties: pallet_bounties = 19,

		// Collator support. The order of these 4 are important and shall not change.
		Authorship: pallet_authorship = 20,
		CollatorStaking: pallet_collator_staking = 21,
		Session: pallet_session = 22,
		Aura: pallet_aura = 23,
		AuraExt: cumulus_pallet_aura_ext = 24,

		// XCM helpers.
		XcmpQueue: cumulus_pallet_xcmp_queue = 30,
		PolkadotXcm: pallet_xcm = 31,
		CumulusXcm: cumulus_pallet_xcm = 32,
		MessageQueue: pallet_message_queue = 33,

		// Other pallets
		Proxy: pallet_proxy = 40,
		Vesting: pallet_vesting = 41,

		Escrow: pallet_escrow = 50,
		MythProxy: pallet_myth_proxy = 51,
		Dmarket: pallet_dmarket = 52,
	}
);

#[cfg(feature = "runtime-benchmarks")]
mod benches {
	frame_benchmarking::define_benchmarks!(
		[cumulus_pallet_parachain_system, ParachainSystem]
		[cumulus_pallet_xcmp_queue, XcmpQueue]
		[cumulus_pallet_weight_reclaim, WeightReclaim]
		[frame_system, SystemBench::<Runtime>]
		[pallet_balances, Balances]
		[pallet_nfts, Nfts]
		[pallet_marketplace, Marketplace]
		[pallet_proxy, Proxy]
		[pallet_escrow, Escrow]
		[pallet_collective, Council]
		[pallet_democracy, Democracy]
		[pallet_dmarket, Dmarket]
		[pallet_escrow, Escrow]
		[pallet_marketplace, Marketplace]
		[pallet_message_queue, MessageQueue]
		[pallet_multibatching, Multibatching]
		[pallet_multisig, Multisig]
		[pallet_myth_proxy, MythProxy]
		[pallet_nfts, Nfts]
		[pallet_preimage, Preimage]
		[pallet_proxy, Proxy]
		[pallet_session, SessionBench::<Runtime>]
		[pallet_scheduler, Scheduler]
		[pallet_sudo, Sudo]
		[pallet_timestamp, Timestamp]
		[pallet_treasury, Treasury]
		[pallet_bounties, Bounties]
		[pallet_vesting, Vesting]
		[pallet_utility, Utility]
		[pallet_identity, Identity]
		[pallet_collator_staking, CollatorStaking]
		[pallet_transaction_payment, TransactionPayment]
	);
}

pub mod genesis_config_presets {
	use super::*;
	use frame_support::build_struct_json_patch;
	use hex_literal::hex;
	use runtime_common::{get_account_id_from_seed, get_collator_keys_from_seed, SAFE_XCM_VERSION};
	use serde_json::{to_string, Value};
	use sp_core::{crypto::UncheckedInto, ecdsa};
	use sp_genesis_builder::{PresetId, DEV_RUNTIME_PRESET};
	use sp_runtime::Percent;

	pub const MUSE_RUNTIME_PRESET: &str = "muse";
	pub const PARA_ID: u32 = 3369;

	fn create_preset(
		invulnerables: Vec<(AccountId, AuraId)>,
		endowed_accounts: Vec<(AccountId, Balance)>,
		council: Vec<AccountId>,
		root_key: AccountId,
		id: ParaId,
	) -> Value {
		build_struct_json_patch!(RuntimeGenesisConfig {
			balances: BalancesConfig { balances: endowed_accounts },
			parachain_info: ParachainInfoConfig { parachain_id: id },
			collator_staking: CollatorStakingConfig {
				invulnerables: invulnerables
					.iter()
					.cloned()
					.map(|(acc, _)| acc)
					.collect::<Vec<_>>(),
				min_candidacy_bond: 50 * MUSE,
				min_stake: 10 * MUSE,
				desired_candidates: 6,
				collator_reward_percentage: Percent::from_parts(10),
				extra_reward: 0,
			},
			council: CouncilConfig { members: council },
			session: SessionConfig {
				keys: invulnerables
					.into_iter()
					.map(|(acc, aura)| {
						(
							acc,                  // account id
							acc,                  // validator id
							SessionKeys { aura }, // session keys
						)
					})
					.collect::<Vec<_>>(),
			},
			sudo: SudoConfig { key: Some(root_key) },
			polkadot_xcm: PolkadotXcmConfig { safe_xcm_version: Some(SAFE_XCM_VERSION) },
		})
	}

	pub fn get_builtin_preset(id: &PresetId) -> Option<Vec<u8>> {
		let preset = match id.as_ref() {
			DEV_RUNTIME_PRESET => {
				let balance_per_account = (1_000_000_000 * MUSE).saturating_div(6);
				create_preset(
					vec![
						(
							get_account_id_from_seed::<ecdsa::Public>("Alice"),
							get_collator_keys_from_seed("Alice"),
						),
						(
							get_account_id_from_seed::<ecdsa::Public>("Bob"),
							get_collator_keys_from_seed("Bob"),
						),
					],
					vec![
						(
							AccountId::from(hex!("f24FF3a9CF04c71Dbc94D0b566f7A27B94566cac")),
							balance_per_account,
						), // Alith
						(
							AccountId::from(hex!("3Cd0A705a2DC65e5b1E1205896BaA2be8A07c6e0")),
							balance_per_account,
						), // Baltathar
						(
							AccountId::from(hex!("798d4Ba9baf0064Ec19eB4F0a1a45785ae9D6DFc")),
							balance_per_account,
						), // Charleth
						(
							AccountId::from(hex!("773539d4Ac0e786233D90A233654ccEE26a613D9")),
							balance_per_account,
						), // Dorothy
						(
							AccountId::from(hex!("Ff64d3F6efE2317EE2807d223a0Bdc4c0c49dfDB")),
							balance_per_account,
						), // Ethan
						(
							AccountId::from(hex!("C0F0f4ab324C46e55D02D0033343B4Be8A55532d")),
							balance_per_account,
						), // Faith
					],
					vec![
						AccountId::from(hex!("3Cd0A705a2DC65e5b1E1205896BaA2be8A07c6e0")), // Baltathar
						AccountId::from(hex!("798d4Ba9baf0064Ec19eB4F0a1a45785ae9D6DFc")), // Charleth
						AccountId::from(hex!("773539d4Ac0e786233D90A233654ccEE26a613D9")), // Dorothy
					],
					AccountId::from(hex!("f24FF3a9CF04c71Dbc94D0b566f7A27B94566cac")),
					PARA_ID.into(),
				)
			},
			MUSE_RUNTIME_PRESET => create_preset(
				vec![
					(
						hex!("e6b4f55209a70384db3d147c06b99e32feb03d6fe191ff62b9dd23d5dd9ac64a")
							.into(),
						hex!("e6b4f55209a70384db3d147c06b99e32feb03d6fe191ff62b9dd23d5dd9ac64a")
							.unchecked_into(),
					),
					(
						hex!("e07113e692708775d0cc39e00fe7f2974bff4e20a6fd127f0810c01142547723")
							.into(),
						hex!("e07113e692708775d0cc39e00fe7f2974bff4e20a6fd127f0810c01142547723")
							.unchecked_into(),
					),
				],
				vec![
					(
						AccountId::from(hex!("16A5094837B65f1177824F0D36002f33d9A2Df7d")),
						150_000_000 * MUSE,
					),
					(
						AccountId::from(hex!("8CC95e7DFa96A86D728D2E6EB86400DEfBB56c90")),
						1_000 * MUSE,
					),
				],
				vec![],
				AccountId::from(hex!("8CC95e7DFa96A86D728D2E6EB86400DEfBB56c90")),
				PARA_ID.into(),
			),
			_ => return None,
		};

		Some(
			to_string(&preset)
				.expect("serialization to json is expected to work. qed.")
				.into_bytes(),
		)
	}

	pub fn preset_names() -> Vec<PresetId> {
		vec![PresetId::from(DEV_RUNTIME_PRESET), PresetId::from(MUSE_RUNTIME_PRESET)]
	}
}

impl_runtime_apis! {
	impl sp_consensus_aura::AuraApi<Block, AuraId> for Runtime {
		fn slot_duration() -> sp_consensus_aura::SlotDuration {
			sp_consensus_aura::SlotDuration::from_millis(SLOT_DURATION)
		}

		fn authorities() -> Vec<AuraId> {
			pallet_aura::Authorities::<Runtime>::get().into_inner()
		}
	}

	impl sp_api::Core<Block> for Runtime {
		fn version() -> RuntimeVersion {
			VERSION
		}

		fn execute_block(block: Block) {
			Executive::execute_block(block)
		}

		fn initialize_block(header: &<Block as BlockT>::Header) -> ExtrinsicInclusionMode {
			Executive::initialize_block(header)
		}
	}

	impl sp_genesis_builder::GenesisBuilder<Block> for Runtime {

		fn get_preset(id: &Option<sp_genesis_builder::PresetId>) -> Option<Vec<u8>> {
			get_preset::<RuntimeGenesisConfig>(id, self::genesis_config_presets::get_builtin_preset)
		}

		fn preset_names() -> Vec<sp_genesis_builder::PresetId> {
			crate::genesis_config_presets::preset_names()
		}

		fn build_state(config: Vec<u8>) -> sp_genesis_builder::Result {
			build_state::<RuntimeGenesisConfig>(config)
		}
	}

	impl sp_api::Metadata<Block> for Runtime {
		fn metadata() -> OpaqueMetadata {
			OpaqueMetadata::new(Runtime::metadata().into())
		}
		fn metadata_at_version(version: u32) -> Option<OpaqueMetadata> {
			Runtime::metadata_at_version(version)
		}

		fn metadata_versions() -> sp_std::vec::Vec<u32> {
			Runtime::metadata_versions()
		}
	}

	impl sp_block_builder::BlockBuilder<Block> for Runtime {
		fn apply_extrinsic(extrinsic: <Block as BlockT>::Extrinsic) -> ApplyExtrinsicResult {
			Executive::apply_extrinsic(extrinsic)
		}

		fn finalize_block() -> <Block as BlockT>::Header {
			Executive::finalize_block()
		}

		fn inherent_extrinsics(data: sp_inherents::InherentData) -> Vec<<Block as BlockT>::Extrinsic> {
			data.create_extrinsics()
		}

		fn check_inherents(
			block: Block,
			data: sp_inherents::InherentData,
		) -> sp_inherents::CheckInherentsResult {
			data.check_extrinsics(&block)
		}
	}

	impl sp_transaction_pool::runtime_api::TaggedTransactionQueue<Block> for Runtime {
		fn validate_transaction(
			source: TransactionSource,
			tx: <Block as BlockT>::Extrinsic,
			block_hash: <Block as BlockT>::Hash,
		) -> TransactionValidity {
			Executive::validate_transaction(source, tx, block_hash)
		}
	}

	impl sp_offchain::OffchainWorkerApi<Block> for Runtime {
		fn offchain_worker(header: &<Block as BlockT>::Header) {
			Executive::offchain_worker(header)
		}
	}

	impl sp_session::SessionKeys<Block> for Runtime {
		fn generate_session_keys(seed: Option<Vec<u8>>) -> Vec<u8> {
			SessionKeys::generate(seed)
		}

		fn decode_session_keys(
			encoded: Vec<u8>,
		) -> Option<Vec<(Vec<u8>, KeyTypeId)>> {
			SessionKeys::decode_into_raw_public_keys(&encoded)
		}
	}

	impl frame_system_rpc_runtime_api::AccountNonceApi<Block, AccountId, Nonce> for Runtime {
		fn account_nonce(account: AccountId) -> Nonce {
			System::account_nonce(account)
		}
	}

	impl pallet_transaction_payment_rpc_runtime_api::TransactionPaymentApi<Block, Balance> for Runtime {
		fn query_info(
			uxt: <Block as BlockT>::Extrinsic,
			len: u32,
		) -> pallet_transaction_payment_rpc_runtime_api::RuntimeDispatchInfo<Balance> {
			TransactionPayment::query_info(uxt, len)
		}
		fn query_fee_details(
			uxt: <Block as BlockT>::Extrinsic,
			len: u32,
		) -> pallet_transaction_payment::FeeDetails<Balance> {
			TransactionPayment::query_fee_details(uxt, len)
		}
		fn query_weight_to_fee(weight: Weight) -> Balance {
			TransactionPayment::weight_to_fee(weight)
		}
		fn query_length_to_fee(length: u32) -> Balance {
			TransactionPayment::length_to_fee(length)
		}
	}

	impl pallet_transaction_payment_rpc_runtime_api::TransactionPaymentCallApi<Block, Balance, RuntimeCall>
		for Runtime
	{
		fn query_call_info(
			call: RuntimeCall,
			len: u32,
		) -> pallet_transaction_payment::RuntimeDispatchInfo<Balance> {
			TransactionPayment::query_call_info(call, len)
		}
		fn query_call_fee_details(
			call: RuntimeCall,
			len: u32,
		) -> pallet_transaction_payment::FeeDetails<Balance> {
			TransactionPayment::query_call_fee_details(call, len)
		}
		fn query_weight_to_fee(weight: Weight) -> Balance {
			TransactionPayment::weight_to_fee(weight)
		}
		fn query_length_to_fee(length: u32) -> Balance {
			TransactionPayment::length_to_fee(length)
		}
	}

	impl pallet_dmarket::DmarketApi<Block, AccountId, Balance, Moment, Hash> for Runtime {
		fn hash_ask_bid_data(trade: TradeParams<Balance, Item, u64>)-> (Hash, Hash) {
			Dmarket::hash_ask_bid_data(&trade)
		}
		fn get_ask_message(caller: AccountId, fee_address: AccountId, trade: TradeParams<Balance, Item, Moment>) -> Vec<u8> {
			Dmarket::get_ask_message(&caller, &fee_address, &trade)
		}
		fn get_bid_message(caller: AccountId, fee_address: AccountId, trade: TradeParams<Balance, Item, Moment>) -> Vec<u8> {
			Dmarket::get_bid_message(&caller, &fee_address, &trade)
		}
	}


	impl cumulus_primitives_aura::AuraUnincludedSegmentApi<Block> for Runtime {
		fn can_build_upon(
			included_hash: <Block as BlockT>::Hash,
			slot: cumulus_primitives_aura::Slot,
		) -> bool {
			ConsensusHook::can_build_upon(included_hash, slot)
		}
	}

	impl cumulus_primitives_core::CollectCollationInfo<Block> for Runtime {
		fn collect_collation_info(header: &<Block as BlockT>::Header) -> cumulus_primitives_core::CollationInfo {
			ParachainSystem::collect_collation_info(header)
		}
	}

	impl pallet_collator_staking::CollatorStakingApi<Block, AccountId, Balance> for Runtime {
		fn main_pot_account() -> AccountId {
			CollatorStaking::account_id()
		}
		fn extra_reward_pot_account() -> AccountId {
			CollatorStaking::extra_reward_account_id()
		}
		fn total_rewards(account: AccountId) -> Balance {
			CollatorStaking::calculate_unclaimed_rewards(&account)
		}
		fn should_claim(account: AccountId) -> bool {
			!CollatorStaking::staker_has_claimed(&account)
		}
		fn candidates() -> Vec<(AccountId, Balance)> {
			CollatorStaking::candidates()
		}
	}

	#[cfg(feature = "try-runtime")]
	impl frame_try_runtime::TryRuntime<Block> for Runtime {
		fn on_runtime_upgrade(checks: frame_try_runtime::UpgradeCheckSelect) -> (Weight, Weight) {
			let weight = Executive::try_runtime_upgrade(checks).unwrap();
			(weight, RuntimeBlockWeights::get().max_block)
		}

		fn execute_block(
			block: Block,
			state_root_check: bool,
			signature_check: bool,
			select: frame_try_runtime::TryStateSelect,
		) -> Weight {
			Executive::try_execute_block(block, state_root_check, signature_check, select).unwrap()
		}
	}

	#[cfg(feature = "runtime-benchmarks")]
	impl frame_benchmarking::Benchmark<Block> for Runtime {
		fn benchmark_metadata(extra: bool) -> (
			Vec<frame_benchmarking::BenchmarkList>,
			Vec<frame_support::traits::StorageInfo>,
		) {
			use frame_benchmarking::BenchmarkList;
			use frame_support::traits::StorageInfoTrait;
			use frame_system_benchmarking::Pallet as SystemBench;
			use cumulus_pallet_session_benchmarking::Pallet as SessionBench;

			let mut list = Vec::<BenchmarkList>::new();
			list_benchmarks!(list, extra);

			let storage_info = AllPalletsWithSystem::storage_info();
			(list, storage_info)
		}

		fn dispatch_benchmark(
			config: frame_benchmarking::BenchmarkConfig
		) -> Result<Vec<frame_benchmarking::BenchmarkBatch>, alloc::string::String> {
			use frame_benchmarking::{BenchmarkError, BenchmarkBatch};

			use frame_system_benchmarking::Pallet as SystemBench;
			impl frame_system_benchmarking::Config for Runtime {
				fn setup_set_code_requirements(code: &sp_std::vec::Vec<u8>) -> Result<(), BenchmarkError> {
					ParachainSystem::initialize_for_set_code_benchmark(code.len() as u32);
					Ok(())
				}

				fn verify_set_code() {
					System::assert_last_event(cumulus_pallet_parachain_system::Event::<Runtime>::ValidationFunctionStored.into());
				}
			}

			use cumulus_pallet_session_benchmarking::Pallet as SessionBench;
			impl cumulus_pallet_session_benchmarking::Config for Runtime {}

			use frame_support::traits::WhitelistedStorageKeys;
			let whitelist = AllPalletsWithSystem::whitelisted_storage_keys();

			let mut batches = Vec::<BenchmarkBatch>::new();
			let params = (&config, &whitelist);
			add_benchmarks!(params, batches);

			if batches.is_empty() { return Err("Benchmark not found for this pallet.".into()) }
			Ok(batches)
		}
	}

	impl xcm_runtime_apis::dry_run::DryRunApi<Block, RuntimeCall, RuntimeEvent, OriginCaller> for Runtime {
		fn dry_run_call(origin: OriginCaller, call: RuntimeCall, result_xcms_version: XcmVersion) -> Result<CallDryRunEffects<RuntimeEvent>, XcmDryRunApiError> {
			PolkadotXcm::dry_run_call::<Runtime, xcm_config::XcmRouter, OriginCaller, RuntimeCall>(origin, call, result_xcms_version)
		}

		fn dry_run_xcm(origin_location: VersionedLocation, xcm: VersionedXcm<RuntimeCall>) -> Result<XcmDryRunEffects<RuntimeEvent>, XcmDryRunApiError> {
			PolkadotXcm::dry_run_xcm::<Runtime, xcm_config::XcmRouter, RuntimeCall, xcm_config::XcmConfig>(origin_location, xcm)
		}
	}
}

cumulus_pallet_parachain_system::register_validate_block! {
	Runtime = Runtime,
	BlockExecutor = cumulus_pallet_aura_ext::BlockExecutor::<Runtime, Executive>,
}
