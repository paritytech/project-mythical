pub(crate) mod xcm_config;

use super::{
	deposit, weights, Aura, Balances, Block, CollatorStaking, ConsensusHook, Escrow, MessageQueue,
	OriginCaller, PalletInfo, ParachainSystem, PolkadotXcm, Preimage, Runtime, RuntimeCall,
	RuntimeEvent, RuntimeFreezeReason, RuntimeHoldReason, RuntimeOrigin, RuntimeTask, Scheduler,
	Session, SessionKeys, System, Treasury, XcmpQueue, EXISTENTIAL_DEPOSIT, MICRO_MUSE, MILLI_MUSE,
	MUSE, VERSION,
};
pub use crate::fee::WeightToFee;
use core::marker::PhantomData;

use cumulus_pallet_parachain_system::RelayNumberMonotonicallyIncreases;
use cumulus_primitives_core::{AggregateMessageOrigin, AssetId, ParaId};

#[cfg(feature = "runtime-benchmarks")]
use pallet_treasury::ArgumentsFactory;
#[cfg(feature = "runtime-benchmarks")]
use sp_core::crypto::FromEntropy;

use sp_core::ConstBool;
use sp_runtime::traits::{BlakeTwo256, IdentityLookup, Verify};

use sp_std::prelude::*;
use sp_version::RuntimeVersion;

use frame_support::traits::{
	fungible,
	fungible::{Balanced, HoldConsideration},
	tokens::{PayFromAccount, UnityAssetBalanceConversion},
	AsEnsureOriginWithArg, InstanceFilter, LinearStoragePrice, OnUnbalanced, WithdrawReasons,
};
use frame_support::{
	derive_impl,
	dispatch::DispatchClass,
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
use pallet_nfts::PalletFeatures;
use parachains_common::message_queue::{NarrowOriginToSibling, ParaIdToSibling};
use parity_scale_codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
pub use runtime_common::{
	AccountId, Balance, BlockNumber, Hash, IncrementableU256, Nonce, Signature,
	AVERAGE_ON_INITIALIZE_RATIO, DAYS, HOURS, MAXIMUM_BLOCK_WEIGHT, MINUTES, NORMAL_DISPATCH_RATIO,
	SLOT_DURATION,
};
pub use sp_consensus_aura::sr25519::AuthorityId as AuraId;
use sp_runtime::traits::ConvertInto;
#[cfg(any(feature = "std", test))]
pub use sp_runtime::BuildStorage;
pub use sp_runtime::{MultiAddress, Perbill, Permill};
use xcm_config::XcmOriginToTransactDispatchOrigin;

// Polkadot imports
use polkadot_runtime_common::{BlockHashCount, SlowAdjustingFeeUpdate};

use weights::{BlockExecutionWeight, ExtrinsicBaseWeight, RocksDbWeight};

// XCM Imports
use crate::configs::xcm_config::SelfReserve;
use runtime_common::AccountIdOf;

/// Privileged origin that represents Root or two thirds of the Council.
pub type RootOrCouncilTwoThirdsMajority = EitherOfDiverse<
	EnsureRoot<AccountId>,
	pallet_collective::EnsureProportionAtLeast<AccountId, CouncilCollective, 2, 3>,
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
	pub const TransactionByteFee: Balance = crate::fee::FEE_MULTIPLIER * 100 * MICRO_MUSE;
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
	type BlockNumberProvider = System;
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
	pub const BondUnlockDelay: BlockNumber = 0;  // previously 5 * MINUTES
	pub const StakeUnlockDelay: BlockNumber = 0;  // previously 2 * MINUTES
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
	type BlockNumberProvider = System;
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
	type BlockNumberProvider = System;
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
	type SpendFunds = ();
	type MaxApprovals = MaxApprovals;
	type SpendOrigin = EnsureWithSuccess<RootOrCouncilTwoThirdsMajority, AccountId, MaxBalance>;
	type AssetKind = ();
	type Beneficiary = AccountId;
	type BeneficiaryLookup = IdentityLookup<Self::Beneficiary>;
	type Paymaster = PayFromAccount<Balances, TreasuryAccount>;
	type BalanceConverter = UnityAssetBalanceConversion;
	type PayoutPeriod = SpendPayoutPeriod;
	type BlockNumberProvider = System;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = TreasuryBenchmarkHelper<Balances>;
}

parameter_types! {
	//   27 | Min encoded size of `Registration`
	// - 10 | Min encoded size of `IdentityInfo`
	// -----|
	//   17 | Min size without `IdentityInfo` (accounted for in byte deposit)
	pub const BasicDeposit: Balance = deposit(1, 17) * 10;
	pub const ByteDeposit: Balance = deposit(0, 1) * 10;
	pub const UsernameDeposit: Balance = deposit(0, 32) * 10;
	pub const SubAccountDeposit: Balance = deposit(1, 53) * 10;
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
