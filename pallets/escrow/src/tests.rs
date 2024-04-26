#![cfg(test)]

use super::*;
use crate::mock::*;
use frame_support::{assert_noop, assert_ok, traits::Currency};

mod escrow {
    use super::*;

    mod deposit {

        use super::*;

        #[test]
        fn should_reserve_deposited_amount() {
            new_test_ext().execute_with(|| {
                let depositor = 1;
                let account_id = 10;

                Balances::make_free_balance_be(&depositor, 1000);
                Balances::make_free_balance_be(&account_id, 100);

                assert_ok!(Escrow::deposit(
                    RuntimeOrigin::signed(depositor),
                    account_id,
                    100,
                    0,
                ));

                assert_eq!(Balances::free_balance(&account_id), 100);
                assert_eq!(Balances::reserved_balance(&account_id), 100);
            });
        }

        #[test]
        fn should_take_currency_from_depositor() {
            new_test_ext().execute_with(|| {
                let depositor = 1;
                let account_id = 10;

                Balances::make_free_balance_be(&depositor, 1000);

                assert_ok!(Escrow::deposit(
                    RuntimeOrigin::signed(depositor),
                    account_id,
                    100,
                    0,
                ));

                assert_eq!(Balances::free_balance(&depositor), 900);
            });
        }

        #[test]
        fn should_emit_event() {
            new_test_ext().execute_with(|| {
                let depositor = 1;
                let account_id = 10;
                let escrow_agent = 2;

                Balances::make_free_balance_be(&depositor, 1000);

                assert_ok!(Escrow::deposit(
                    RuntimeOrigin::signed(depositor),
                    account_id,
                    100,
                    escrow_agent,
                ));

                System::assert_last_event(
                    Event::Deposited {
                        account: account_id,
                        value: 100,
                        agent: escrow_agent,
                    }
                    .into(),
                );
            });
        }

        #[test]
        fn consiquent_deposits_should_increase_total_deposited() {
            new_test_ext().execute_with(|| {
                let depositor = 1;
                let account_id = 10;
                let escrow_agent = 2;

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

                assert_eq!(total_deposited(&account_id), 200);
            });
        }

        #[test]
        fn should_keep_min_balance_free() {
            new_test_ext().execute_with(|| {
                let depositor = 1;
                let account_id = 10;
                Balances::make_free_balance_be(&depositor, 1000);

                assert_ok!(Escrow::deposit(
                    RuntimeOrigin::signed(depositor),
                    account_id,
                    100,
                    0,
                ));

                assert_eq!(Balances::free_balance(account_id), 1);
                assert_eq!(Balances::reserved_balance(account_id), 99);

                assert_eq!(total_deposited(&account_id), 99);
            });
        }

        #[test]
        fn should_fail_if_value_less_then_minimum() {
            new_test_ext().execute_with(|| {
                let depositor = 1;
                let account_id = 10;
                Balances::make_free_balance_be(&depositor, 1000);

                assert_noop!(
                    Escrow::deposit(RuntimeOrigin::signed(depositor), account_id, 0, 0),
                    Error::<Test>::DepositTooSmall
                );
            });
        }
    }

    mod releaes {
        use super::*;

        #[test]
        fn should_unreserve_balance() {
            new_test_ext().execute_with(|| {
                let depositor = 1;
                let account_id = 10;
                let escrow_agent = 2;

                Balances::make_free_balance_be(&depositor, 1000);
                Balances::make_free_balance_be(&account_id, 100);

                assert_ok!(Escrow::deposit(
                    RuntimeOrigin::signed(depositor),
                    account_id,
                    100,
                    escrow_agent
                ));

                assert_ok!(Escrow::release(
                    RuntimeOrigin::signed(escrow_agent),
                    account_id,
                    100,
                ));

                assert_eq!(Balances::free_balance(&account_id), 200);
            });
        }

        #[test]
        fn should_fail_for_unauthorized_origin() {
            new_test_ext().execute_with(|| {
                let depositor = 1;
                let account_id = 10;
                let escrow_agent = 2;

                Balances::make_free_balance_be(&depositor, 1000);
                Balances::make_free_balance_be(&account_id, 100);

                assert_ok!(Escrow::deposit(
                    RuntimeOrigin::signed(depositor),
                    account_id,
                    100,
                    escrow_agent
                ));

                assert_noop!(
                    Escrow::release(RuntimeOrigin::signed(1), account_id, 100),
                    Error::<Test>::Unauthorized
                );
            });
        }

        #[test]
        fn should_emit_event() {
            new_test_ext().execute_with(|| {
                let depositor = 1;
                let account_id = 10;
                let escrow_agent = 2;

                Balances::make_free_balance_be(&depositor, 1000);
                Balances::make_free_balance_be(&account_id, 100);

                assert_ok!(Escrow::deposit(
                    RuntimeOrigin::signed(depositor),
                    account_id,
                    100,
                    escrow_agent
                ));

                assert_ok!(Escrow::release(
                    RuntimeOrigin::signed(escrow_agent),
                    account_id,
                    100,
                ));

                System::assert_last_event(
                    Event::Released {
                        account: account_id,
                        value: 100,
                    }
                    .into(),
                );
            });
        }

        #[test]
        fn should_not_release_more_than_deposited() {
            new_test_ext().execute_with(|| {
                let depositor = 1;
                let account_id = 10;
                let escrow_agent = 2;

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
                let depositor = 1;
                let account_id = 10;
                let escrow_agent = 2;

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

                assert_eq!(Balances::free_balance(&account_id), 100);
                assert_eq!(Balances::free_balance(&depositor), 1000);
            });
        }

        #[test]
        fn should_emit_event() {
            new_test_ext().execute_with(|| {
                let depositor = 1;
                let account_id = 10;
                let escrow_agent = 2;

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
                        reason: reason,
                    }
                    .into(),
                );
            });
        }

        #[test]
        fn should_fail_for_unauthorized_origin() {
            new_test_ext().execute_with(|| {
                let depositor = 1;
                let account_id = 10;
                let escrow_agent = 2;

                Balances::make_free_balance_be(&depositor, 1000);
                Balances::make_free_balance_be(&account_id, 100);

                assert_ok!(Escrow::deposit(
                    RuntimeOrigin::signed(depositor),
                    account_id,
                    100,
                    escrow_agent
                ));

                assert_noop!(
                    Escrow::revoke(RuntimeOrigin::signed(1), account_id, depositor, vec![]),
                    Error::<Test>::Unauthorized
                );
            });
        }
    }
}
