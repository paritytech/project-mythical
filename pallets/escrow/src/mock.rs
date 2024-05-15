#![cfg(test)]

use super::*;

use crate as pallet_escrow;
use account::AccountId20;
use frame_support::{derive_impl, traits::ConstU64};
use sp_runtime::{traits::IdentityLookup, BuildStorage};

type Block = frame_system::mocking::MockBlock<Test>;

const MIN_DEPOSIT: BalanceOf<Test> = 2;

frame_support::construct_runtime!(
	pub enum Test
	{
		System: frame_system,
		Balances: pallet_balances,
		Escrow: pallet_escrow,
	}
);

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
	type Balance = BalanceOf<Test>;
	type MinDeposit = ConstU64<MIN_DEPOSIT>;
	type RuntimeHoldReason = RuntimeHoldReason;
	type WeightInfo = ();
}

pub fn new_test_ext() -> sp_io::TestExternalities {
	let t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();

	let mut ext = sp_io::TestExternalities::new(t);
	ext.execute_with(|| System::set_block_number(1));
	ext
}
