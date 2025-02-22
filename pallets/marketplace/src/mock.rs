use frame_support::{
	derive_impl,
	pallet_prelude::DispatchResult,
	parameter_types,
	traits::{
		fungible::Mutate, AsEnsureOriginWithArg, ConstU128, ConstU32, ConstU64,
		NamedReservableCurrency,
	},
};
use frame_system as system;
use sp_core::H256;
use sp_keystore::{testing::MemoryKeystore, KeystoreExt};
use sp_runtime::{
	traits::{BlakeTwo256, IdentifyAccount, IdentityLookup, Verify},
	BuildStorage,
};

use account::EthereumSignature;

use crate::{self as pallet_marketplace};
use pallet_nfts::PalletFeatures;

type Signature = EthereumSignature;
pub type AccountId = <<Signature as Verify>::Signer as IdentifyAccount>::AccountId;
type Block = frame_system::mocking::MockBlock<Test>;

pub const ESCROW_RESERVE_NAME: &[u8; 8] = b"escrow__";

// Configure a mock runtime to test the pallet.
frame_support::construct_runtime!(
	pub enum Test
	{
		System: frame_system,
		Marketplace: pallet_marketplace,
		Balances: pallet_balances,
		Timestamp: pallet_timestamp,
		Nfts: pallet_nfts,
	}
);

parameter_types! {
	pub const BlockHashCount: u64 = 250;
	pub const SS58Prefix: u8 = 42;
}

#[derive_impl(frame_system::config_preludes::TestDefaultConfig as frame_system::DefaultConfig)]
impl frame_system::Config for Test {
	type BaseCallFilter = frame_support::traits::Everything;
	type BlockWeights = ();
	type BlockLength = ();
	type RuntimeOrigin = RuntimeOrigin;
	type RuntimeCall = RuntimeCall;
	type Nonce = u64;
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type AccountId = AccountId;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Block = Block;
	type RuntimeEvent = RuntimeEvent;
	type BlockHashCount = ConstU64<250>;
	type DbWeight = ();
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = pallet_balances::AccountData<u128>;
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = ();
	type OnSetCode = ();
	type MaxConsumers = ConstU32<16>;
}

parameter_types! {
	pub storage Features: PalletFeatures = PalletFeatures::all_enabled();
}

impl pallet_nfts::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type CollectionId = u32;
	type Currency = Balances;
	type CreateOrigin = AsEnsureOriginWithArg<frame_system::EnsureSigned<Self::AccountId>>;
	type ForceOrigin = frame_system::EnsureRoot<Self::AccountId>;
	type Locker = ();
	type CollectionDeposit = ConstU128<2>;
	type ItemDeposit = ConstU128<1>;
	type MetadataDepositBase = ConstU128<1>;
	type AttributeDepositBase = ConstU128<1>;
	type DepositPerByte = ConstU128<1>;
	type StringLimit = ConstU32<50>;
	type KeyLimit = ConstU32<50>;
	type ValueLimit = ConstU32<50>;
	type ApprovalsLimit = ConstU32<10>;
	type ItemAttributesApprovalsLimit = ConstU32<2>;
	type MaxTips = ConstU32<10>;
	type MaxDeadlineDuration = ConstU64<10000>;
	type MaxAttributesPerCall = ConstU32<2>;
	type Features = Features;
	type OffchainSignature = Signature;
	type OffchainPublic = <Signature as Verify>::Signer;
	type WeightInfo = ();
	pallet_nfts::runtime_benchmarks_enabled! {
		type Helper = ();
	}
}

pub struct EscrowMock {
	pub deposit: u128,
}

impl pallet_marketplace::Escrow<AccountId, u128, AccountId> for EscrowMock {
	fn make_deposit(
		depositor: &AccountId,
		destination: &AccountId,
		value: u128,
		_escrow_agent: &AccountId,
	) -> DispatchResult {
		Balances::transfer(
			depositor,
			destination,
			value,
			frame_support::traits::tokens::Preservation::Expendable,
		)?;

		Balances::reserve_named(ESCROW_RESERVE_NAME, destination, value)?;

		Ok(())
	}
}

impl pallet_marketplace::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type RuntimeCall = RuntimeCall;
	type Currency = Balances;
	type Escrow = EscrowMock;
	type RuntimeHoldReason = RuntimeHoldReason;
	type MinOrderDuration = ConstU64<10>;
	type NonceStringLimit = ConstU32<50>;
	type Signature = Signature;
	type Signer = <Signature as Verify>::Signer;
	type WeightInfo = ();
	pallet_marketplace::runtime_benchmarks_enabled! {
		type BenchmarkHelper = ();
	}
}

impl pallet_balances::Config for Test {
	type MaxLocks = ();
	type MaxReserves = ConstU32<2>;
	type ReserveIdentifier = [u8; 8];
	type Balance = u128;
	type DustRemoval = ();
	type RuntimeEvent = RuntimeEvent;
	type ExistentialDeposit = ConstU128<1>;
	type AccountStore = System;
	type WeightInfo = ();
	type RuntimeHoldReason = RuntimeHoldReason;
	type RuntimeFreezeReason = ();
	type FreezeIdentifier = ();
	type MaxFreezes = ();
	type DoneSlashHandler = ();
}

impl pallet_timestamp::Config for Test {
	type Moment = u64;
	type OnTimestampSet = ();
	type MinimumPeriod = ConstU64<3>;
	type WeightInfo = ();
}

// Build genesis storage according to the mock runtime.
pub fn new_test_ext() -> sp_io::TestExternalities {
	let t = system::GenesisConfig::<Test>::default().build_storage().unwrap();
	let mut ext = sp_io::TestExternalities::new(t);
	ext.register_extension(KeystoreExt::new(MemoryKeystore::new()));
	ext.execute_with(|| System::set_block_number(1));
	ext
}
