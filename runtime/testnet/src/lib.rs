#![cfg_attr(not(feature = "std"), no_std)]
// `construct_runtime!` does a lot of recursion and requires us to increase the limit to 256.
#![recursion_limit = "256"]

// Make the WASM binary available.
#[cfg(feature = "std")]
include!(concat!(env!("OUT_DIR"), "/wasm_binary.rs"));

mod weights;
pub mod xcm_config;
pub use fee::WeightToFee;

use cumulus_pallet_parachain_system::RelayNumberMonotonicallyIncreases;
use cumulus_primitives_core::{AggregateMessageOrigin, AssetId, ParaId};
use sp_api::impl_runtime_apis;
use sp_core::{crypto::KeyTypeId, ConstBool, OpaqueMetadata, H160};
use sp_runtime::{
	create_runtime_str, generic, impl_opaque_keys,
	traits::{BlakeTwo256, Block as BlockT, Verify},
	transaction_validity::{TransactionSource, TransactionValidity},
	ApplyExtrinsicResult, ExtrinsicInclusionMode,
};

use sp_std::prelude::*;
#[cfg(feature = "std")]
use sp_version::NativeVersion;
use sp_version::RuntimeVersion;

use frame_support::traits::{AsEnsureOriginWithArg, InstanceFilter, WithdrawReasons};
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
	EnsureRoot, EnsureSignedBy,
};
use pallet_nfts::PalletFeatures;
use parachains_common::message_queue::{NarrowOriginToSibling, ParaIdToSibling};
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
pub use runtime_common::{
	AccountId, Balance, BlockNumber, DealWithFees, Hash, IncrementableU256, Nonce, Signature,
	AVERAGE_ON_INITIALIZE_RATIO, DAYS, HOURS, MAXIMUM_BLOCK_WEIGHT, MINUTES, NORMAL_DISPATCH_RATIO,
	SLOT_DURATION,
};
pub use sp_consensus_aura::sr25519::AuthorityId as AuraId;
use sp_runtime::traits::ConvertInto;
pub use sp_runtime::{MultiAddress, Perbill, Permill};
use xcm_config::XcmOriginToTransactDispatchOrigin;

#[cfg(any(feature = "std", test))]
pub use sp_runtime::BuildStorage;

// Polkadot imports
use polkadot_runtime_common::{BlockHashCount, SlowAdjustingFeeUpdate};

use weights::{BlockExecutionWeight, ExtrinsicBaseWeight, RocksDbWeight};

// XCM Imports
use crate::xcm_config::SelfReserve;
use xcm::latest::prelude::BodyId;

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

/// The SignedExtension to the basic transaction logic.
pub type SignedExtra = (
	frame_system::CheckNonZeroSender<Runtime>,
	frame_system::CheckSpecVersion<Runtime>,
	frame_system::CheckTxVersion<Runtime>,
	frame_system::CheckGenesis<Runtime>,
	frame_system::CheckEra<Runtime>,
	frame_system::CheckNonce<Runtime>,
	frame_system::CheckWeight<Runtime>,
	pallet_transaction_payment::ChargeTransactionPayment<Runtime>,
	cumulus_primitives_storage_weight_reclaim::StorageWeightReclaim<Runtime>,
);

/// Unchecked extrinsic type as expected by this runtime.
pub type UncheckedExtrinsic =
	fp_self_contained::UncheckedExtrinsic<Address, RuntimeCall, Signature, SignedExtra>;

/// Extrinsic type that has already been checked.
pub type CheckedExtrinsic =
	fp_self_contained::CheckedExtrinsic<AccountId, RuntimeCall, SignedExtra, H160>;

/// Executive: handles dispatch to the various modules.
pub type Executive = frame_executive::Executive<
	Runtime,
	Block,
	frame_system::ChainContext<Runtime>,
	Runtime,
	AllPalletsWithSystem,
>;

pub mod fee {

	use super::{Balance, ExtrinsicBaseWeight, MILLI_MUSE, MILLI_ROC};
	use frame_support::weights::{
		constants::WEIGHT_REF_TIME_PER_SECOND, FeePolynomial, Weight, WeightToFeeCoefficient,
		WeightToFeeCoefficients, WeightToFeePolynomial,
	};
	use smallvec::smallvec;
	use sp_runtime::Perbill;

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
			ref_fee.max(proof_fee)
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
			// Map 10kb proof to 1/10 MILLI_MYTH.
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
	spec_name: create_runtime_str!("muse"),
	impl_name: create_runtime_str!("muse"),
	authoring_version: 1,
	spec_version: 1010,
	impl_version: 0,
	apis: RUNTIME_API_VERSIONS,
	transaction_version: 1,
	state_version: 1,
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
	type EventHandler = (CollatorSelection,);
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
	type FreezeIdentifier = ();
	type MaxLocks = ConstU32<50>;
	type MaxReserves = ConstU32<50>;
	type MaxFreezes = ConstU32<0>;
	type RuntimeFreezeReason = RuntimeFreezeReason;
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
	/// Relay Chain `TransactionByteFee` / 10
	pub const TransactionByteFee: Balance = 10 * MICRO_MUSE;
}

impl pallet_transaction_payment::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type OnChargeTransaction =
		pallet_transaction_payment::FungibleAdapter<Balances, DealWithFees<Runtime>>;
	type OperationalFeeMultiplier = ConstU8<5>;
	type WeightToFee = WeightToFee;
	type LengthToFee = ConstantMultiplier<Balance, TransactionByteFee>;
	type FeeMultiplierUpdate = SlowAdjustingFeeUpdate<Self>;
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

parameter_types! {
	pub const Period: u32 = 6 * HOURS;
	pub const Offset: u32 = 0;
}

impl pallet_session::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type ValidatorId = <Self as frame_system::Config>::AccountId;
	// we don't have stash and controller, thus we don't need the convert as well.
	type ValidatorIdOf = pallet_collator_selection::IdentityCollator;
	type ShouldEndSession = pallet_session::PeriodicSessions<Period, Offset>;
	type NextSessionRotation = pallet_session::PeriodicSessions<Period, Offset>;
	type SessionManager = CollatorSelection;
	// Essentially just Aura, but lets be pedantic.
	type SessionHandler = <SessionKeys as sp_runtime::traits::OpaqueKeys>::KeyTypeIdProviders;
	type Keys = SessionKeys;
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
	pub const MaxCandidates: u32 = 100;
	pub const MinEligibleCollators: u32 = 5;
	pub const SessionLength: BlockNumber = 6 * HOURS;
	pub const MaxInvulnerables: u32 = 20;
	// StakingAdmin pluralistic body.
	pub const StakingAdminBodyId: BodyId = BodyId::Defense;
}

impl pallet_collator_selection::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type UpdateOrigin = RootOrCouncilTwoThirdsMajority;
	type PotId = PotId;
	type MaxCandidates = MaxCandidates;
	type MinEligibleCollators = MinEligibleCollators;
	type MaxInvulnerables = MaxInvulnerables;
	// should be a multiple of session or things will get inconsistent
	type KickThreshold = Period;
	type ValidatorId = <Self as frame_system::Config>::AccountId;
	type ValidatorIdOf = pallet_collator_selection::IdentityCollator;
	type ValidatorRegistration = Session;
	type WeightInfo = weights::pallet_collator_selection::WeightInfo<Runtime>;
}

// Project specific pallets.

parameter_types! {
	pub NftsPalletFeatures: PalletFeatures = PalletFeatures::all_enabled();
	pub const NftsMaxDeadlineDuration: BlockNumber = 12 * 30 * DAYS;
	pub const NftsCollectionDeposit: Balance = 0;
	pub const NftsItemDeposit: Balance = 0;
	pub const NftsMetadataDepositBase: Balance = 0;
	pub const NftsAttributeDepositBase: Balance = 0;
	pub const NftsDepositPerByte: Balance = 0;
}

pub type CollectionId = IncrementableU256;

impl pallet_nfts::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type CollectionId = CollectionId;
	type Currency = Balances;
	type CreateOrigin = AsEnsureOriginWithArg<frame_system::EnsureSigned<AccountId>>;
	type ForceOrigin = EnsureRoot<AccountId>;
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
					| RuntimeCall::MythProxy(pallet_myth_proxy::Call::approve_proxy_funding { .. })
					| RuntimeCall::MythProxy(
						pallet_myth_proxy::Call::register_sponsor_agent { .. }
					) | RuntimeCall::MythProxy(pallet_myth_proxy::Call::revoke_sponsor_agent { .. })
			),
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
}

// Create the runtime by composing the FRAME pallets that were previously configured.
construct_runtime!(
	pub struct Runtime {
		// System support stuff.
		System: frame_system = 0,
		ParachainSystem: cumulus_pallet_parachain_system = 1,
		Timestamp: pallet_timestamp = 2,
		ParachainInfo: parachain_info = 3,

		// Utility
		Utility: pallet_utility = 4,
		Multisig: pallet_multisig = 5,

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

		// Collator support. The order of these 4 are important and shall not change.
		Authorship: pallet_authorship = 20,
		CollatorSelection: pallet_collator_selection = 21,
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
	}
);

impl fp_self_contained::SelfContainedCall for RuntimeCall {
	type SignedInfo = sp_core::H160;

	//Ethereum calls dont exist in this runtime
	fn is_self_contained(&self) -> bool {
		match self {
			_ => false,
		}
	}

	fn check_self_contained(
		&self,
	) -> Option<Result<Self::SignedInfo, sp_runtime::transaction_validity::TransactionValidityError>>
	{
		match self {
			_ => None,
		}
	}

	fn validate_self_contained(
		&self,
		_info: &Self::SignedInfo,
		_dispatch_info: &sp_runtime::traits::DispatchInfoOf<RuntimeCall>,
		_len: usize,
	) -> Option<sp_runtime::transaction_validity::TransactionValidity> {
		match self {
			_ => None,
		}
	}

	fn pre_dispatch_self_contained(
		&self,
		_info: &Self::SignedInfo,
		_dispatch_info: &sp_runtime::traits::DispatchInfoOf<RuntimeCall>,
		_len: usize,
	) -> Option<Result<(), sp_runtime::transaction_validity::TransactionValidityError>> {
		match self {
			_ => None,
		}
	}

	fn apply_self_contained(
		self,
		_info: Self::SignedInfo,
	) -> Option<sp_runtime::DispatchResultWithInfo<sp_runtime::traits::PostDispatchInfoOf<Self>>> {
		match self {
			_ => None,
		}
	}
}

#[cfg(feature = "runtime-benchmarks")]
mod benches {
	frame_benchmarking::define_benchmarks!(
		[frame_system, SystemBench::<Runtime>]
		[pallet_timestamp, Timestamp]
		[pallet_balances, Balances]
		[pallet_multibatching, Multibatching]
		[pallet_utility, Utility]
		[cumulus_pallet_parachain_system, ParachainSystem]
		[pallet_message_queue, MessageQueue]
		[cumulus_pallet_xcmp_queue, XcmpQueue]
		[pallet_session, SessionBench::<Runtime>]
		[pallet_sudo, Sudo]
		[pallet_multisig, Multisig]
		[pallet_collator_selection, CollatorSelection]
		[pallet_nfts, Nfts]
		[pallet_marketplace, Marketplace]
		[pallet_proxy, Proxy]
		[pallet_escrow, Escrow]
		[pallet_vesting, Vesting]
		[pallet_collective, Council]
		[pallet_myth_proxy, MythProxy]
	);
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
			get_preset::<RuntimeGenesisConfig>(id, |_| None)
		}

		fn preset_names() -> Vec<sp_genesis_builder::PresetId> {
			vec![]
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
			use frame_benchmarking::{Benchmarking, BenchmarkList};
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
		) -> Result<Vec<frame_benchmarking::BenchmarkBatch>, sp_runtime::RuntimeString> {
			use frame_benchmarking::{BenchmarkError, Benchmarking, BenchmarkBatch};

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
}

cumulus_pallet_parachain_system::register_validate_block! {
	Runtime = Runtime,
	BlockExecutor = cumulus_pallet_aura_ext::BlockExecutor::<Runtime, Executive>,
}
