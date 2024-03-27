use core::{marker::PhantomData, ops::ControlFlow};

use crate::fee::default_fee_per_second;
use frame_support::traits::{Contains, ContainsPair, Get};
use frame_support::{
	parameter_types,
	traits::{ConstU32, Everything, Nothing, ProcessMessageError},
};
use frame_system::EnsureRoot;
use hex_literal::hex;
use pallet_xcm::XcmPassthrough;
use parachains_common::xcm_config::ParentRelayOrSiblingParachains;
use polkadot_runtime_common::xcm_sender::ExponentialPrice;
use sp_std::vec::Vec;
use xcm::latest::prelude::*;
use xcm_builder::{
	AccountKey20Aliases, AllowExplicitUnpaidExecutionFrom, AllowKnownQueryResponses,
	AllowSubscriptionsFrom, AllowTopLevelPaidExecutionFrom, CreateMatcher, DescribeFamily,
	DescribeTerminus, EnsureXcmOrigin, FixedRateOfFungible, FixedWeightBounds,
	FrameTransactionalProcessor, FungibleAdapter, HashedDescription, IsConcrete, MatchXcm,
	NativeAsset, RelayChainAsNative, SiblingParachainAsNative, SovereignSignedViaLocation,
	TakeWeightCredit, TrailingSetTopicAsId, UsingComponents, WithComputedOrigin, WithUniqueTopic,
};
use xcm_executor::traits::Properties;
use xcm_executor::{traits::ShouldExecute, XcmExecutor};

use runtime_common::DealWithFees;
use xcm_primitives::SignedToAccountId20;

use super::{
	AccountId, AllPalletsWithSystem, Balances, BaseDeliveryFee, FeeAssetId, ParachainInfo,
	ParachainSystem, PolkadotXcm, Runtime, RuntimeCall, RuntimeEvent, RuntimeOrigin,
	TransactionByteFee, WeightToFee, XcmpQueue,
};

const ASSET_HUB_PARA_ID: u32 = 1000;

parameter_types! {
	pub const RelayLocation: Location = Location::parent();
	pub const RelayNetwork: NetworkId = NetworkId::Polkadot;
	pub const SelfReserve: Location = Location::here();
	pub RelayChainOrigin: RuntimeOrigin = cumulus_pallet_xcm::Origin::Relay.into();
	pub UniversalLocation: InteriorLocation =
		[GlobalConsensus(RelayNetwork::get()), Parachain(ParachainInfo::parachain_id().into())].into();
	pub EthereumCurrencyLocation: Location = Location::new(2,
		[
			GlobalConsensus(NetworkId::Ethereum { chain_id: 1 }), // mainnet
			// MYTHOS ERC20
			AccountKey20 { network: None, key: hex!("BA41Ddf06B7fFD89D1267b5A93BFeF2424eb2003") }
		]);
	// Arbitrary value to allow to test reserve transfers, only for testing.
	// pub EthereumCurrencyLocation: Location = Location::new(1, [Parachain(2001)]);
}

/// Type for specifying how a `Location` can be converted into an `AccountId`. This is used
/// when determining ownership of accounts for asset transacting and when attempting to use XCM
/// `Transact` in order to determine the dispatch Origin.
pub type LocationToAccountId = (
	// The parent (Relay-chain) origin converts to the parent `AccountId`.
	// Sibling parachain origins convert to AccountId via the `ParaId::into`.
	HashedDescription<AccountId, DescribeFamily<DescribeTerminus>>,
	// Straight up local `AccountId32` origins just alias directly to `AccountId`.
	AccountKey20Aliases<RelayNetwork, AccountId>,
);

/// Means for transacting the native currency on this chain.
pub type LocalAssetTransactor = FungibleAdapter<
	// Use this currency:
	Balances,
	// Use this currency when it is a fungible asset matching the given location or name:
	IsConcrete<SelfReserve>,
	// Convert an XCM Location into a local account id:
	LocationToAccountId,
	// Our chain's account ID type (we can't get away without mentioning it explicitly):
	AccountId,
	// We allow issuance to be modified on teleport.
	(),
>;

/// Means for transacting the native currency on this chain with an Ethereum token on sepolia
pub type BridgedLocalAssetTransactor = FungibleAdapter<
	// Use this currency:
	Balances,
	// Use this currency when it is a fungible asset matching the given location or name:
	IsConcrete<EthereumCurrencyLocation>,
	// Convert an XCM Location into a local account id:
	LocationToAccountId,
	// Our chain's account ID type (we can't get away without mentioning it explicitly):
	AccountId,
	// We don't track any teleports.
	(),
>;

/// Means for transacting assets on this chain.
pub type AssetTransactors = (LocalAssetTransactor, BridgedLocalAssetTransactor);

/// This is the type we use to convert an (incoming) XCM origin into a local `Origin` instance,
/// ready for dispatching a transaction with Xcm's `Transact`. There is an `OriginKind` which can
/// biases the kind of local `Origin` it will become.
pub type XcmOriginToTransactDispatchOrigin = (
	// Sovereign account converter; this attempts to derive an `AccountId` from the origin location
	// using `LocationToAccountId` and then turn that into the usual `Signed` origin. Useful for
	// foreign chains who want to have a local sovereign account on this chain which they control.
	SovereignSignedViaLocation<LocationToAccountId, RuntimeOrigin>,
	// Native converter for Relay-chain (Parent) location; will convert to a `Relay` origin when
	// recognized.
	RelayChainAsNative<RelayChainOrigin, RuntimeOrigin>,
	// Native converter for sibling Parachains; will convert to a `SiblingPara` origin when
	// recognized.
	SiblingParachainAsNative<cumulus_pallet_xcm::Origin, RuntimeOrigin>,
	// Xcm origins can be represented natively under the Xcm pallet's Xcm origin.
	XcmPassthrough<RuntimeOrigin>,
);

parameter_types! {
	// One XCM operation is 1_000_000_000 weight - almost certainly a conservative estimate.
	pub UnitWeightCost: Weight = Weight::from_parts(1_000_000_000, 64 * 1024);
	pub const MaxInstructions: u32 = 100;
	pub const MaxAssetsIntoHolding: u32 = 64;
}

pub struct ParentOrParentsExecutivePlurality;
impl Contains<Location> for ParentOrParentsExecutivePlurality {
	fn contains(l: &Location) -> bool {
		matches!(l.unpack(), (1, []) | (1, [Plurality { id: BodyId::Executive, .. }]))
	}
}

//TODO: move DenyThenTry to polkadot's xcm module.
/// Deny executing the xcm message if it matches any of the Deny filter regardless of anything else.
/// If it passes the Deny, and matches one of the Allow cases then it is let through.
pub struct DenyThenTry<Deny, Allow>(PhantomData<Deny>, PhantomData<Allow>)
where
	Deny: ShouldExecute,
	Allow: ShouldExecute;

impl<Deny, Allow> ShouldExecute for DenyThenTry<Deny, Allow>
where
	Deny: ShouldExecute,
	Allow: ShouldExecute,
{
	fn should_execute<RuntimeCall>(
		origin: &Location,
		instructions: &mut [Instruction<RuntimeCall>],
		max_weight: Weight,
		properties: &mut Properties,
	) -> Result<(), ProcessMessageError> {
		Deny::should_execute(origin, instructions, max_weight, properties)?;
		Allow::should_execute(origin, instructions, max_weight, properties)
	}
}

// See issue <https://github.com/paritytech/polkadot/issues/5233>
pub struct DenyReserveTransferToRelayChain;
impl ShouldExecute for DenyReserveTransferToRelayChain {
	fn should_execute<RuntimeCall>(
		origin: &Location,
		instructions: &mut [Instruction<RuntimeCall>],
		_max_weight: Weight,
		_properties: &mut Properties,
	) -> Result<(), ProcessMessageError> {
		instructions.matcher().match_next_inst_while(
			|_| true,
			|inst| match inst {
				InitiateReserveWithdraw {
					reserve: Location { parents: 1, interior: Here },
					..
				}
				| DepositReserveAsset { dest: Location { parents: 1, interior: Here }, .. }
				| TransferReserveAsset { dest: Location { parents: 1, interior: Here }, .. } => {
					Err(ProcessMessageError::Unsupported) // Deny
				},
				// An unexpected reserve transfer has arrived from the Relay Chain. Generally,
				// `IsReserve` should not allow this, but we just log it here.
				ReserveAssetDeposited { .. }
					if matches!(origin, Location { parents: 1, interior: Here }) =>
				{
					log::warn!(
						target: "xcm::barrier",
						"Unexpected ReserveAssetDeposited from the Relay Chain",
					);
					Ok(ControlFlow::Continue(()))
				},
				_ => Ok(ControlFlow::Continue(())),
			},
		)?;

		// Permit everything else
		Ok(())
	}
}

pub type Barrier = TrailingSetTopicAsId<
	DenyThenTry<
		DenyReserveTransferToRelayChain,
		(
			TakeWeightCredit,
			// Expected responses are OK.
			AllowKnownQueryResponses<PolkadotXcm>,
			// Allow XCMs with some computed origins to pass through.
			WithComputedOrigin<
				(
					// If the message is one that immediately attempts to pay for execution, then
					// allow it.
					AllowTopLevelPaidExecutionFrom<OnlyAssetHubPrefix>,
					// Parent, its pluralities (i.e. governance bodies), and the Fellows plurality
					// get free execution.
					AllowExplicitUnpaidExecutionFrom<ParentOrParentsExecutivePlurality>,
					// Subscriptions for version tracking are OK.
					AllowSubscriptionsFrom<ParentRelayOrSiblingParachains>,
				),
				UniversalLocation,
				ConstU32<8>,
			>,
		),
	>,
>;

parameter_types! {
	pub AssetHubLocation: Location = Location::new(1, [Parachain(ASSET_HUB_PARA_ID)]);
	// ALWAYS ensure that the index in PalletInstance stays up-to-date with
	// AssetHub's `ForeignAssets` pallet index
	pub AssetHubAssetsPalletLocation: Location =
		Location::new(1, [Parachain(ASSET_HUB_PARA_ID), PalletInstance(53)]);
	pub const NativeAssetId: AssetId = AssetId(SelfReserve::get());
	pub const NativeAssetFilter: AssetFilter = Wild(AllOf { fun: WildFungible, id: NativeAssetId::get() });
	pub AssetHubTrustedTeleporter: (AssetFilter, Location) = (NativeAssetFilter::get(), AssetHubLocation::get());
	pub RelayPerSecondAndByte: (AssetId, u128,u128) = (Location::new(1,Here).into(), default_fee_per_second() * 1, 1024);
}

pub struct OnlyAssetHubPrefix;
impl Contains<Location> for OnlyAssetHubPrefix {
	fn contains(location: &Location) -> bool {
		matches!(
			location.unpack(),
			(1, [Parachain(ASSET_HUB_PARA_ID)]) | (1, [Parachain(ASSET_HUB_PARA_ID), _])
		)
	}
}

pub struct ReserveAssetsFrom<T>(PhantomData<T>);
impl<T: Get<Location>> ContainsPair<Asset, Location> for ReserveAssetsFrom<T> {
	fn contains(asset: &Asset, origin: &Location) -> bool {
		let prefix = T::get();
		log::trace!(target: "xcm::ReserveAssetsFrom", "prefix: {:?}, origin: {:?}, asset: {:?}", prefix, origin, asset);
		asset.id != NativeAssetId::get() && &prefix == origin
	}
}

pub struct OnlyTeleportNative;
impl Contains<(Location, Vec<Asset>)> for OnlyTeleportNative {
	fn contains(t: &(Location, Vec<Asset>)) -> bool {
		let native = SelfReserve::get();
		t.1.iter().all(|asset| {
			log::trace!(target: "xcm::OnlyTeleportNative", "Asset to be teleported: {:?}", asset);
			if let Asset { id: asset_id, fun: Fungible(_) } = asset {
				asset_id.0 == native
			} else {
				false
			}
		})
	}
}

pub type Traders = (
	//Relay token.
	FixedRateOfFungible<RelayPerSecondAndByte, ()>,
	//Native asset.
	UsingComponents<WeightToFee, SelfReserve, AccountId, Balances, DealWithFees<Runtime>>,
);

pub type Reserves = (NativeAsset, ReserveAssetsFrom<AssetHubLocation>);
pub type TrustedTeleporters = (xcm_builder::Case<AssetHubTrustedTeleporter>,);

pub struct XcmConfig;
impl xcm_executor::Config for XcmConfig {
	type RuntimeCall = RuntimeCall;
	type XcmSender = XcmRouter;
	type AssetTransactor = AssetTransactors;
	type OriginConverter = XcmOriginToTransactDispatchOrigin;
	type IsReserve = Reserves;
	type IsTeleporter = TrustedTeleporters;
	type Aliasers = Nothing;
	type UniversalLocation = UniversalLocation;
	type Barrier = Barrier;
	//TODO: Replace with benchmarked weights
	type Weigher = FixedWeightBounds<UnitWeightCost, RuntimeCall, MaxInstructions>;
	type Trader = Traders;
	type ResponseHandler = PolkadotXcm;
	type AssetTrap = PolkadotXcm;
	type AssetLocker = ();
	type AssetExchanger = ();
	type AssetClaims = PolkadotXcm;
	type SubscriptionService = PolkadotXcm;
	type PalletInstancesInfo = AllPalletsWithSystem;
	type MaxAssetsIntoHolding = MaxAssetsIntoHolding;
	//Currently fees are being burned.
	type FeeManager = ();
	type MessageExporter = ();
	type UniversalAliases = Nothing;
	type CallDispatcher = RuntimeCall;
	// Disallow Transacts execution.
	type SafeCallFilter = Nothing;
	type TransactionalProcessor = FrameTransactionalProcessor;
}

/// Local origin to location conversion.
pub type LocalOriginToLocation = SignedToAccountId20<RuntimeOrigin, AccountId, RelayNetwork>;

pub type PriceForParentDelivery =
	ExponentialPrice<FeeAssetId, BaseDeliveryFee, TransactionByteFee, ParachainSystem>;

/// The means for routing XCM messages which are not for local execution into the right message
/// queues.
pub type XcmRouter = WithUniqueTopic<(
	// Two routers - use UMP to communicate with the relay chain:
	cumulus_primitives_utility::ParentAsUmp<ParachainSystem, PolkadotXcm, PriceForParentDelivery>,
	// and XCMP to communicate with the sibling chains.
	XcmpQueue,
)>;

impl pallet_xcm::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	// We disallow users to send arbitrary XCMs from this chain. Root can send.
	type SendXcmOrigin = EnsureXcmOrigin<RuntimeOrigin, ()>;
	type AdminOrigin = EnsureRoot<AccountId>;
	type XcmRouter = XcmRouter;
	// We must allow execution for running XCM programs to integrate with other chains.
	type ExecuteXcmOrigin = EnsureXcmOrigin<RuntimeOrigin, LocalOriginToLocation>;
	// We enable executing until setup of integration with other chains via XCM is done.
	type XcmExecuteFilter = Everything;
	type XcmExecutor = XcmExecutor<XcmConfig>;
	// Only teleport the native asset
	type XcmTeleportFilter = OnlyTeleportNative;
	// All reserve transfers are allowed.
	type XcmReserveTransferFilter = Everything;
	// Use (conservative) bounds on estimating XCM execution on this chain.
	//TODO: Replace with benchmarked weights
	type Weigher = FixedWeightBounds<UnitWeightCost, RuntimeCall, MaxInstructions>;
	type UniversalLocation = UniversalLocation;
	type RuntimeOrigin = RuntimeOrigin;
	type RuntimeCall = RuntimeCall;

	// Override for AdvertisedXcmVersion default
	const VERSION_DISCOVERY_QUEUE_SIZE: u32 = 100;
	type AdvertisedXcmVersion = pallet_xcm::CurrentXcmVersion;
	type Currency = Balances;
	type CurrencyMatcher = ();
	type TrustedLockers = ();
	type SovereignAccountOf = LocationToAccountId;
	type MaxLockers = ConstU32<8>;
	//TODO: Replace with benchmarked weights
	type WeightInfo = pallet_xcm::TestWeightInfo;
	type MaxRemoteLockConsumers = ConstU32<0>;
	type RemoteLockConsumerIdentifier = ();
}

impl cumulus_pallet_xcm::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type XcmExecutor = XcmExecutor<XcmConfig>;
}
