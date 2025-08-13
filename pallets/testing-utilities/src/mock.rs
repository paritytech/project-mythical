#![cfg(test)]

use super::*;

use crate as pallet_testing_utilities;
use account::AccountId20;
use frame_support::{derive_impl, parameter_types};
use frame_system;
use sp_runtime::{
	traits::IdentityLookup,
	BuildStorage,
};

type Block = frame_system::mocking::MockBlock<Test>;

// Configure a mock runtime to test the pallet.
frame_support::construct_runtime! {
	pub enum Test
	{
		System: frame_system,
		Balances: pallet_balances,
		TestingUtilities: pallet_testing_utilities,
	}
}

parameter_types! {
	pub const ExistentialDeposit: u128 = 500;
	pub const MaxLocks: u32 = 50;
}

#[derive_impl(frame_system::config_preludes::TestDefaultConfig as frame_system::DefaultConfig)]
impl frame_system::Config for Test {
	type Block = Block;
	type BaseCallFilter = frame_support::traits::Everything;
	type AccountData = pallet_balances::AccountData<u64>;
	type AccountId = AccountId20;
	type Lookup = IdentityLookup<AccountId20>;
}

#[derive_impl(pallet_balances::config_preludes::TestDefaultConfig as pallet_balances::DefaultConfig)]
impl pallet_balances::Config for Test {
	type ReserveIdentifier = [u8; 8];
	type AccountStore = System;
}

impl Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type BlockNumberProvider = frame_system::Pallet<Test>;
	type WeightInfo = SubstrateWeight<Test>;
}

pub fn new_test_ext() -> sp_io::TestExternalities {
	let t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();

	let mut ext = sp_io::TestExternalities::new(t);
	ext.execute_with(|| System::set_block_number(1));
	ext
}
