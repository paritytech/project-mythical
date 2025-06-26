#![cfg(test)]

use super::*;
use crate::mock::*;
use account::AccountId20;
use frame_support::{assert_noop, assert_ok, traits::Currency};
use sp_runtime::traits::Zero;

type AccountIdOf<T> = <T as frame_system::Config>::AccountId;

pub fn total_deposited<T: pallet::Config>(account: &AccountIdOf<T>) -> BalanceOf<T> {
	Deposits::<T>::iter_prefix_values(account).fold(Zero::zero(), |acc, d| acc + d)
}

fn account(id: u8) -> AccountId20 {
	[id; 20].into()
}

mod escrow {
	use super::*;

	mod deposit {

		use super::*;

		#[test]
		fn should_reserve_deposited_amount() {
			new_test_ext().execute_with(|| {
				let depositor = account(1);
				let account_id = account(10);
				let escrow_agent = account(2);

				Balances::make_free_balance_be(&depositor, 1000);
				Balances::make_free_balance_be(&account_id, 100);

				assert_ok!(Escrow::deposit(
					RuntimeOrigin::signed(depositor),
					account_id,
					100,
					escrow_agent
				));

				assert_eq!(Balances::free_balance(account_id), 100);
				assert_eq!(Balances::reserved_balance(account_id), 100);
			});
		}

		#[test]
		fn should_take_currency_from_depositor() {
			new_test_ext().execute_with(|| {
				let depositor = account(1);
				let account_id = account(10);
				let escrow_agent = account(2);

				Balances::make_free_balance_be(&depositor, 1000);
				Balances::make_free_balance_be(&account_id, 1);

				assert_ok!(Escrow::deposit(
					RuntimeOrigin::signed(depositor),
					account_id,
					100,
					escrow_agent
				));

				assert_eq!(Balances::free_balance(depositor), 900);
			});
		}

		#[test]
		fn should_emit_event() {
			new_test_ext().execute_with(|| {
				let depositor = account(1);
				let account_id = account(10);
				let escrow_agent = account(2);

				Balances::make_free_balance_be(&depositor, 1000);
				Balances::make_free_balance_be(&account_id, 1);

				assert_ok!(Escrow::deposit(
					RuntimeOrigin::signed(depositor),
					account_id,
					100,
					escrow_agent,
				));

				System::assert_last_event(
					Event::Deposited { account: account_id, value: 100, agent: escrow_agent }
						.into(),
				);
			});
		}

		#[test]
		fn consiquent_deposits_should_increase_total_deposited() {
			new_test_ext().execute_with(|| {
				let depositor = account(1);
				let account_id = account(10);
				let escrow_agent = account(2);

				Balances::make_free_balance_be(&depositor, 1000);
				Balances::make_free_balance_be(&account_id, 100);

				assert_ok!(Escrow::deposit(
					RuntimeOrigin::signed(depositor),
					account_id,
					100,
					escrow_agent,
				));

				assert_ok!(Escrow::deposit(
					RuntimeOrigin::signed(depositor),
					account_id,
					100,
					escrow_agent,
				));

				assert_eq!(total_deposited::<Test>(&account_id), 200);
			});
		}

		#[test]
		fn should_fail_if_balance_less_then_minimum() {
			new_test_ext().execute_with(|| {
				let depositor = account(1);
				let account_id = account(10);
				let escrow_agent = account(2);

				Balances::make_free_balance_be(&depositor, 1000);

				assert_noop!(
					Escrow::deposit(
						RuntimeOrigin::signed(depositor),
						account_id,
						100,
						escrow_agent
					),
					Error::<Test>::BalanceTooLow
				);
			});
		}

		#[test]
		fn should_fail_if_value_less_then_minimum() {
			new_test_ext().execute_with(|| {
				let depositor = account(1);
				let account_id = account(10);
				let escrow_agent = account(2);

				Balances::make_free_balance_be(&depositor, 1000);

				assert_noop!(
					Escrow::deposit(RuntimeOrigin::signed(depositor), account_id, 0, escrow_agent),
					Error::<Test>::DepositTooLow
				);
			});
		}
	}

	mod release {
		use super::*;

		#[test]
		fn should_unreserve_balance() {
			new_test_ext().execute_with(|| {
				let depositor = account(1);
				let account_id = account(10);
				let escrow_agent = account(2);

				Balances::make_free_balance_be(&depositor, 1000);
				Balances::make_free_balance_be(&account_id, 100);

				assert_ok!(Escrow::deposit(
					RuntimeOrigin::signed(depositor),
					account_id,
					100,
					escrow_agent
				));

				assert_ok!(Escrow::release(RuntimeOrigin::signed(escrow_agent), account_id, 100,));

				assert_eq!(Balances::free_balance(account_id), 200);
			});
		}

		#[test]
		fn should_fail_for_unauthorized_origin() {
			new_test_ext().execute_with(|| {
				let depositor = account(1);
				let account_id = account(10);
				let escrow_agent = account(2);

				Balances::make_free_balance_be(&depositor, 1000);
				Balances::make_free_balance_be(&account_id, 100);

				assert_ok!(Escrow::deposit(
					RuntimeOrigin::signed(depositor),
					account_id,
					100,
					escrow_agent
				));

				assert_noop!(
					Escrow::release(RuntimeOrigin::signed(account_id), account_id, 100),
					Error::<Test>::NoSuchDeposit
				);
			});
		}

		#[test]
		fn should_emit_event() {
			new_test_ext().execute_with(|| {
				let depositor = account(1);
				let account_id = account(10);
				let escrow_agent = account(2);

				Balances::make_free_balance_be(&depositor, 1000);
				Balances::make_free_balance_be(&account_id, 100);

				assert_ok!(Escrow::deposit(
					RuntimeOrigin::signed(depositor),
					account_id,
					100,
					escrow_agent
				));

				assert_ok!(Escrow::release(RuntimeOrigin::signed(escrow_agent), account_id, 100,));

				System::assert_last_event(
					Event::Released { account: account_id, value: 100, agent: escrow_agent }.into(),
				);
			});
		}

		#[test]
		fn should_not_release_more_than_deposited() {
			new_test_ext().execute_with(|| {
				let depositor = account(1);
				let account_id = account(10);
				let escrow_agent = account(2);

				Balances::make_free_balance_be(&depositor, 1000);
				Balances::make_free_balance_be(&account_id, 100);

				assert_ok!(Escrow::deposit(
					RuntimeOrigin::signed(depositor),
					account_id,
					100,
					escrow_agent
				));

				assert_noop!(
					Escrow::release(RuntimeOrigin::signed(escrow_agent), account_id, 101),
					Error::<Test>::InsufficientBalance
				);
			});
		}
	}

	mod revoke {
		use super::*;

		#[test]
		fn should_transfer_reserved_funds() {
			new_test_ext().execute_with(|| {
				let depositor = account(1);
				let account_id = account(10);
				let escrow_agent = account(2);

				Balances::make_free_balance_be(&depositor, 1000);
				Balances::make_free_balance_be(&account_id, 100);

				assert_ok!(Escrow::deposit(
					RuntimeOrigin::signed(depositor),
					account_id,
					100,
					escrow_agent
				));

				assert_ok!(Escrow::revoke(
					RuntimeOrigin::signed(escrow_agent),
					account_id,
					depositor,
					vec![],
				));

				assert_eq!(Balances::free_balance(account_id), 100);
				assert_eq!(Balances::free_balance(depositor), 1000);
				assert_eq!(total_deposited::<Test>(&account_id), 0);
			});
		}

		#[test]
		fn should_emit_event() {
			new_test_ext().execute_with(|| {
				let depositor = account(1);
				let account_id = account(10);
				let escrow_agent = account(2);

				Balances::make_free_balance_be(&depositor, 1000);
				Balances::make_free_balance_be(&account_id, 100);

				assert_ok!(Escrow::deposit(
					RuntimeOrigin::signed(depositor),
					account_id,
					100,
					escrow_agent
				));

				let reason = "Rewoke reason".as_bytes().to_vec();

				assert_ok!(Escrow::revoke(
					RuntimeOrigin::signed(escrow_agent),
					account_id,
					depositor,
					reason.clone(),
				));

				System::assert_last_event(
					Event::Revoked {
						account: account_id,
						destination: depositor,
						agent: escrow_agent,
						value: 100,
						reason,
					}
					.into(),
				);
			});
		}

		#[test]
		fn should_fail_for_unauthorized_origin() {
			new_test_ext().execute_with(|| {
				let depositor = account(1);
				let account_id = account(10);
				let escrow_agent = account(2);

				Balances::make_free_balance_be(&depositor, 1000);
				Balances::make_free_balance_be(&account_id, 100);

				assert_ok!(Escrow::deposit(
					RuntimeOrigin::signed(depositor),
					account_id,
					100,
					escrow_agent
				));

				assert_noop!(
					Escrow::revoke(
						RuntimeOrigin::signed(account_id),
						account_id,
						depositor,
						vec![]
					),
					Error::<Test>::NoSuchDeposit
				);
			});
		}
	}

	mod force_release {
		use super::*;

		#[test]
		fn allows_sudo_to_release_funds() {
			new_test_ext().execute_with(|| {
				let depositor = account(1);
				let account_id = account(10);
				let escrow_agent = account(2);

				Balances::make_free_balance_be(&depositor, 1000);
				Balances::make_free_balance_be(&account_id, 100);

				assert_ok!(Escrow::deposit(
					RuntimeOrigin::signed(depositor),
					account_id,
					100,
					escrow_agent,
				));

				assert_ok!(Escrow::force_release(
					RuntimeOrigin::root(),
					account_id,
					escrow_agent,
					100,
				));

				assert_eq!(Balances::free_balance(account_id), 200);
			});
		}
	}
}
