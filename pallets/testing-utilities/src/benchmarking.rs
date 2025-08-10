//#![cfg(feature = "runtime-benchmarks")]

use crate::*;

use crate::Pallet as TestingUtilities;
use account::AccountId20;
use frame_benchmarking::v2::*;
use frame_support::{traits::{Currency, Hooks}};
use frame_system::RawOrigin;
use pallet_balances::Pallet as Balances;
use sp_runtime::traits::{Block, Header};

#[benchmarks(
    where
		<T as frame_system::Config>::AccountId: From<AccountId20> + Into<AccountId20>,
        <T as frame_system::Config>::RuntimeOrigin: From<RawOrigin<T::AccountId>>,
		<<<T as frame_system::Config>
			::Block as Block>
			::Header as Header>
			::Number: From<u64>,
        <T as pallet_balances::Config>::Balance: From<BalanceOf<T>> + From<u64>,
		<T as Config>::Balance: From<u64>,
)]
mod benchmarks {
	use super::*;

	#[benchmark]
	fn transfer_through_delayed_remint() {
		let from: T::AccountId = whitelisted_caller();
		let to: T::AccountId = account("recipient", 0, 0);
		let amount: BalanceOf<T> = 9001_u64.into();

		Balances::<T>::make_free_balance_be(&from, 10_000_u64.into());
		Balances::<T>::make_free_balance_be(&to,   10_000_u64.into());

		#[extrinsic_call]
		_(RawOrigin::Signed(from), to.clone(), amount);

		TestingUtilities::<T>::on_idle(2_u64.into(), Weight::MAX);
	}

	impl_benchmark_test_suite! {
		TestingUtilities,
		crate::mock::new_test_ext(),
		crate::mock::Test,
	}
}
