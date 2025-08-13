#![cfg(feature = "runtime-benchmarks")]

use account::AccountId20;
use frame_benchmarking::v2::*;
use frame_support::{assert_ok, traits::{Currency, Hooks}};
use frame_system::RawOrigin;
use pallet_balances::Pallet as Balances;
use sp_runtime::traits::{Block, Header};

use crate::{*, Pallet as TestingUtilities};

#[benchmarks(
    where
		<T as frame_system::Config>::AccountId: From<AccountId20> + Into<AccountId20>,
        <T as frame_system::Config>::RuntimeOrigin: From<RawOrigin<T::AccountId>>,
		<T as pallet_balances::Config>::Balance: From<BalanceOf<T>>,
		<<<T as frame_system::Config>::Block as Block>::Header as Header>::Number: From<u32>,
)]
mod benchmarks {
	use super::*;

	#[benchmark]
	fn transfer_through_delayed_remint() {
		let ed = <T as Config>::Currency::minimum_balance();
		let initial_amount: BalanceOf<T> = 10u8.into();
		let transfer_amount: BalanceOf<T> = 1u8.into();

		let from: T::AccountId = whitelisted_caller();
		let to: T::AccountId = account("recipient", 0, 0);
		let amount: BalanceOf<T> = (transfer_amount * ed).into();

		Balances::<T>::make_free_balance_be(&from, (initial_amount * ed).into());
		Balances::<T>::make_free_balance_be(&to,   (initial_amount * ed).into());

		#[extrinsic_call]
		_(RawOrigin::Signed(from), to.clone(), amount);

		TestingUtilities::<T>::on_idle(2_u32.into(), Weight::MAX);
	}

	#[benchmark]
	fn execute_scheduled_transfer() {
		let ed = <T as Config>::Currency::minimum_balance();
		let initial_amount: BalanceOf<T> = 10u8.into();
		let transfer_amount: BalanceOf<T> = 1u8.into();

		let from: T::AccountId = whitelisted_caller();
		let to: T::AccountId = account("recipient", 0, 0);
		let amount: BalanceOf<T> = (transfer_amount * ed).into();

		Balances::<T>::make_free_balance_be(&from, (initial_amount * ed).into());
		Balances::<T>::make_free_balance_be(&to,   (initial_amount * ed).into());

		assert_ok!(
			TestingUtilities::<T>::transfer_through_delayed_remint(RawOrigin::Signed(from).into(), to.clone(), amount)
		);

		#[block]
		{
			assert_eq!(TestingUtilities::<T>::execute_scheduled_transfer(), true);
		}
	}

	impl_benchmark_test_suite! {
		TestingUtilities,
		crate::mock::new_test_ext(),
		crate::mock::Test,
	}
}
