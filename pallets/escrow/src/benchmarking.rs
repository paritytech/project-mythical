#![cfg(feature = "runtime-benchmarks")]

use crate::*;

use crate::Pallet as Escrow;
use account::AccountId20;
use frame_benchmarking::v2::*;
use frame_support::{assert_ok, traits::Currency};
use frame_system::RawOrigin;
use pallet_balances::Pallet as Balances;

#[benchmarks(
    where
		<T as frame_system::Config>::AccountId: From<AccountId20> + Into<AccountId20>,
        <T as pallet_balances::Config>::Balance: From<BalanceOf<T>>,
        <T as frame_system::Config>::RuntimeOrigin: From<RawOrigin<T::AccountId>>,
)]
mod benchmarks {
	use super::*;

	#[benchmark]
	fn deposit() {
		let depositor: T::AccountId = whitelisted_caller();
		let account_id: T::AccountId = account("account", 0, 0);
		let escrow_agent: T::AccountId = account("escrow_agent", 0, 0);

		let min_deposit = Escrow::<T>::min_deposit();

		let initial_depositor_balance: BalanceOf<T> = min_deposit * 100u32.into();
		let initial_account_balance: BalanceOf<T> = min_deposit * 10u32.into();

		let deposit_value: BalanceOf<T> = min_deposit * 10u32.into();

		Balances::<T>::make_free_balance_be(&depositor, initial_depositor_balance.into());
		Balances::<T>::make_free_balance_be(&account_id, initial_account_balance.into());

		#[extrinsic_call]
		_(RawOrigin::Signed(depositor), account_id.clone(), deposit_value, escrow_agent.clone());

		assert_eq!(Balances::<T>::free_balance(&account_id), initial_account_balance.into());
		assert_eq!(Balances::<T>::reserved_balance(&account_id), deposit_value.into());
		assert_eq!(Escrow::<T>::get_deposit(&account_id, &escrow_agent), deposit_value);
	}

	#[benchmark]
	fn release() {
		let depositor: T::AccountId = whitelisted_caller();
		let account_id: T::AccountId = account("account", 0, 0);
		let escrow_agent: T::AccountId = account("escrow_agent", 0, 0);

		let initial_depositor_balance = min_deposit_times::<T>(100);
		let initial_account_balance = min_deposit_times::<T>(10);

		let deposit_value = min_deposit_times::<T>(10);
		let release_value = min_deposit_times::<T>(5);

		Balances::<T>::make_free_balance_be(&depositor, initial_depositor_balance.into());
		Balances::<T>::make_free_balance_be(&account_id, initial_account_balance.into());

		assert_ok!(Escrow::<T>::deposit(
			RawOrigin::Signed(depositor).into(),
			account_id.clone(),
			deposit_value,
			escrow_agent.clone(),
		));

		#[extrinsic_call]
		_(RawOrigin::Signed(escrow_agent.clone()), account_id.clone(), release_value);

		assert_eq!(
			Balances::<T>::free_balance(&account_id),
			(initial_account_balance + release_value).into()
		);

		assert_eq!(
			Balances::<T>::reserved_balance(&account_id),
			(deposit_value - release_value).into()
		);
		assert_eq!(
			Escrow::<T>::get_deposit(&account_id, &escrow_agent),
			deposit_value - release_value
		);
	}

	#[benchmark]
	fn revoke() {
		let depositor: T::AccountId = whitelisted_caller();
		let account_id: T::AccountId = account("account", 0, 0);
		let escrow_agent: T::AccountId = account("escrow_agent", 0, 0);

		let initial_depositor_balance = min_deposit_times::<T>(100);
		let initial_account_balance = min_deposit_times::<T>(10);

		let deposit_value = min_deposit_times::<T>(10);

		Balances::<T>::make_free_balance_be(&depositor, initial_depositor_balance.into());
		Balances::<T>::make_free_balance_be(&account_id, initial_account_balance.into());

		assert_ok!(Escrow::<T>::deposit(
			RawOrigin::Signed(depositor.clone()).into(),
			account_id.clone(),
			deposit_value,
			escrow_agent.clone(),
		));

		let revoke_reason = "Rewoke reason".as_bytes().to_vec();

		#[extrinsic_call]
		_(RawOrigin::Signed(escrow_agent.clone()), account_id.clone(), depositor, revoke_reason);

		assert_eq!(Balances::<T>::free_balance(&account_id), initial_account_balance.into());

		assert_eq!(Balances::<T>::reserved_balance(&account_id), 0u32.into());
		assert_eq!(Escrow::<T>::get_deposit(&account_id, &escrow_agent), 0u32.into());
	}

	#[benchmark]
	fn force_release() {
		let depositor: T::AccountId = whitelisted_caller();
		let account_id: T::AccountId = account("account", 0, 0);
		let escrow_agent: T::AccountId = account("escrow_agent", 0, 0);

		let initial_depositor_balance = min_deposit_times::<T>(100);
		let initial_account_balance = min_deposit_times::<T>(10);

		let deposit_value = min_deposit_times::<T>(10);
		let release_value = min_deposit_times::<T>(5);

		Balances::<T>::make_free_balance_be(&depositor, initial_depositor_balance.into());
		Balances::<T>::make_free_balance_be(&account_id, initial_account_balance.into());

		assert_ok!(Escrow::<T>::deposit(
			RawOrigin::Signed(depositor).into(),
			account_id.clone(),
			deposit_value,
			escrow_agent.clone(),
		));

		#[extrinsic_call]
		_(RawOrigin::Root, account_id.clone(), escrow_agent.clone(), release_value);

		assert_eq!(
			Balances::<T>::free_balance(&account_id),
			(initial_account_balance + release_value).into()
		);

		assert_eq!(
			Balances::<T>::reserved_balance(&account_id),
			(deposit_value - release_value).into()
		);
		assert_eq!(
			Escrow::<T>::get_deposit(&account_id, &escrow_agent),
			deposit_value - release_value
		);
	}

	fn min_deposit_times<T: Config>(n: u32) -> BalanceOf<T> {
		Escrow::<T>::min_deposit() * n.into()
	}

	impl_benchmark_test_suite! {
		Escrow,
		crate::mock::new_test_ext(),
		crate::mock::Test,
	}
}
