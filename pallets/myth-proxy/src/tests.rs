use super::*;
use crate::mock::*;
use frame_support::{assert_noop, assert_ok};
use sp_runtime::{ArithmeticError, DispatchErrorWithPostInfo};

macro_rules! assert_proxy_error {
	($proxy_call:expr, $expected_error:expr) => {{
		let (origin, delegator, inner_call) = $proxy_call;

		let outer_call =
			RuntimeCall::Proxy(crate::Call::proxy { address: delegator, call: inner_call });
		let info = outer_call.get_dispatch_info();
		let result = outer_call.dispatch(origin);

		assert_noop!(
			result,
			DispatchErrorWithPostInfo {
				error: $expected_error.into(),
				post_info: PostDispatchInfo {
					actual_weight: Some(info.call_weight),
					pays_fee: Pays::Yes
				},
			}
		);
	}};
}

fn call_transfer(dest: u64, value: u64) -> RuntimeCall {
	RuntimeCall::Balances(pallet_balances::Call::transfer_keep_alive { dest, value })
}

fn make_free_balance_be(account: &u64, balance: u64) {
	assert_ok!(<Test as pallet::Config>::Currency::mint_into(account, balance));
}

mod proxy {
	use super::*;

	#[test]
	fn creation_should_store_proxy() {
		new_test_ext().execute_with(|| {
			let delegator = 1;
			let delegate = 2;

			make_free_balance_be(&delegator, 10);

			assert_ok!(Proxy::add_proxy(
				RuntimeOrigin::signed(delegator),
				delegate,
				ProxyType::Any,
				None,
			));

			assert!(Proxy::has_proxy(&delegator, &delegate));
			System::assert_last_event(
				Event::ProxyCreated {
					delegator,
					delegate,
					proxy_type: ProxyType::Any,
					sponsor: None,
				}
				.into(),
			);
		});
	}

	#[test]
	fn should_reserve_deposit() {
		new_test_ext().execute_with(|| {
			let delegator = 1;
			let delegate = 2;

			make_free_balance_be(&delegator, 10);

			assert_ok!(Proxy::add_proxy(
				RuntimeOrigin::signed(delegator),
				delegate,
				ProxyType::Any,
				None,
			));

			assert_eq!(Balances::reserved_balance(delegator), 1);
		});
	}

	#[test]
	fn delegator_can_remove_proxy() {
		new_test_ext().execute_with(|| {
			let delegator = 1;
			let delegate = 2;

			make_free_balance_be(&delegator, 10);

			assert_ok!(Proxy::add_proxy(
				RuntimeOrigin::signed(delegator),
				delegate,
				ProxyType::Any,
				None,
			));

			assert_ok!(Proxy::remove_proxy(RuntimeOrigin::signed(delegator), delegate,));

			assert!(!Proxy::has_proxy(&delegator, &delegate));

			System::assert_last_event(
				Event::ProxyRemoved { delegator, delegate, removed_by_sponsor: None }.into(),
			);
		});
	}

	#[test]
	fn removing_should_free_deposit() {
		new_test_ext().execute_with(|| {
			let delegator = 1;
			let delegate = 2;

			make_free_balance_be(&delegator, 10);

			assert_ok!(Proxy::add_proxy(
				RuntimeOrigin::signed(delegator),
				delegate,
				ProxyType::Any,
				None,
			));

			assert_eq!(Balances::reserved_balance(delegator), 1);

			assert_ok!(Proxy::remove_proxy(RuntimeOrigin::signed(delegator), delegate,));

			assert_eq!(Balances::reserved_balance(delegator), 0);
		});
	}

	mod proxying {
		use super::*;

		#[test]
		fn delegate_can_make_proxied_calls() {
			new_test_ext().execute_with(|| {
				let delegator = 1;
				let delegate = 2;
				let third_party = 10;

				make_free_balance_be(&delegator, 10);

				assert_ok!(Proxy::add_proxy(
					RuntimeOrigin::signed(delegator),
					delegate,
					ProxyType::Any,
					None,
				));

				assert!(Proxy::has_proxy(&delegator, &delegate));

				let call = Box::new(call_transfer(third_party, 1));

				assert_ok!(Proxy::proxy(RuntimeOrigin::signed(delegate), delegator, call));

				System::assert_last_event(Event::ProxyExecuted { delegator, delegate }.into());

				assert_eq!(Balances::free_balance(third_party), 1);
			});
		}

		#[test]
		fn should_fail_if_internal_call_fails() {
			new_test_ext().execute_with(|| {
				let delegator = 1;
				let delegate = 2;
				let third_party = 10;

				make_free_balance_be(&delegator, 10);

				assert_ok!(Proxy::add_proxy(
					RuntimeOrigin::signed(delegator),
					delegate,
					ProxyType::Any,
					None,
				));

				let call = Box::new(call_transfer(third_party, 11));

				assert_proxy_error!(
					(RuntimeOrigin::signed(delegate), delegator, call),
					ArithmeticError::Underflow
				);
			});
		}

		#[test]
		fn should_fail_if_not_proxy() {
			new_test_ext().execute_with(|| {
				let delegator = 1;
				let delegate = 2;
				let third_party = 10;

				let call = Box::new(call_transfer(third_party, 1));

				assert_noop!(
					Proxy::proxy(RuntimeOrigin::signed(delegate), delegator, call),
					Error::<Test>::NotProxy
				);
			});
		}

		#[test]
		fn should_not_allow_delegate_to_modify_proxy_permissions() {
			new_test_ext().execute_with(|| {
				let delegator = 1;
				let delegate = 2;
				let third_party = 10;

				make_free_balance_be(&delegator, 10);

				assert_ok!(Proxy::add_proxy(
					RuntimeOrigin::signed(delegator),
					delegate,
					ProxyType::NoModifyProxy,
					None,
				));

				let call = Box::new(RuntimeCall::Proxy(crate::Call::add_proxy {
					delegate: third_party,
					proxy_type: ProxyType::Any,
					sponsor: None,
				}));

				assert_proxy_error!(
					(RuntimeOrigin::signed(delegate), delegator, call),
					frame_system::Error::<Test>::CallFiltered
				);
			});
		}

		#[test]
		fn should_not_allow_modify_proxy_with_higher_permissions() {
			new_test_ext().execute_with(|| {
				let delegator = 1;
				let delegate_root = 2;
				let delegate = 3;
				let third_party = 10;

				make_free_balance_be(&delegator, 10);

				assert_ok!(Proxy::add_proxy(
					RuntimeOrigin::signed(delegator),
					delegate_root,
					ProxyType::Any,
					None,
				));

				assert_ok!(Proxy::add_proxy(
					RuntimeOrigin::signed(delegator),
					delegate,
					ProxyType::NoBalances,
					None,
				));

				assert_proxy_error!(
					(
						RuntimeOrigin::signed(delegate),
						delegator,
						Box::new(RuntimeCall::Proxy(crate::Call::remove_proxy {
							delegate: delegate_root
						}))
					),
					frame_system::Error::<Test>::CallFiltered
				);

				assert_proxy_error!(
					(
						RuntimeOrigin::signed(delegate),
						delegator,
						Box::new(RuntimeCall::Proxy(crate::Call::add_proxy {
							delegate: third_party,
							proxy_type: ProxyType::Any,
							sponsor: None,
						}))
					),
					frame_system::Error::<Test>::CallFiltered
				);
			});
		}

		#[test]
		fn can_be_restricted_with_proxy_type() {
			new_test_ext().execute_with(|| {
				let delegator = 1;
				let delegate = 2;
				let third_party = 10;

				make_free_balance_be(&delegator, 10);

				assert_ok!(Proxy::add_proxy(
					RuntimeOrigin::signed(delegator),
					delegate,
					ProxyType::NoBalances,
					None,
				));

				let call = Box::new(call_transfer(third_party, 1));

				assert_proxy_error!(
					(RuntimeOrigin::signed(delegate), delegator, call),
					frame_system::Error::<Test>::CallFiltered
				);
			})
		}

		#[test]
		fn editing_can_be_restricted_with_proxy_type() {
			new_test_ext().execute_with(|| {
				let delegator = 1;
				let delegate = 2;

				make_free_balance_be(&delegator, 10);

				assert_ok!(Proxy::add_proxy(
					RuntimeOrigin::signed(delegator),
					delegate,
					ProxyType::NoModifyProxy,
					None,
				));

				let call = Box::new(RuntimeCall::Proxy(crate::Call::remove_proxy { delegate }));

				assert_proxy_error!(
					(RuntimeOrigin::signed(delegate), delegator, call),
					frame_system::Error::<Test>::CallFiltered
				);
			})
		}
	}

	mod sponsorship {
		use super::*;

		#[test]
		fn deposit_can_be_sponsored() {
			new_test_ext().execute_with(|| {
				let delegator = 1;
				let delegate = 2;
				let sponsor = 3;
				let sponsor_agent = 4;

				make_free_balance_be(&sponsor, 10);

				assert_ok!(Proxy::register_sponsor_agent(
					RuntimeOrigin::signed(sponsor),
					sponsor_agent
				));

				assert_ok!(Proxy::approve_proxy_funding(
					RuntimeOrigin::signed(sponsor_agent),
					sponsor,
					delegator,
				));

				assert_ok!(Proxy::add_proxy(
					RuntimeOrigin::signed(delegator),
					delegate,
					ProxyType::Any,
					Some(sponsor),
				));

				assert_eq!(Balances::reserved_balance(sponsor), 1);
			});
		}

		#[test]
		fn approval_should_be_removed_after_first_use() {
			new_test_ext().execute_with(|| {
				let delegator = 1;
				let delegate = 2;
				let sponsor = 3;
				let sponsor_agent = 4;

				make_free_balance_be(&sponsor, 10);

				assert_ok!(Proxy::register_sponsor_agent(
					RuntimeOrigin::signed(sponsor),
					sponsor_agent
				));

				assert_ok!(Proxy::approve_proxy_funding(
					RuntimeOrigin::signed(sponsor_agent),
					sponsor,
					delegator,
				));

				assert_ok!(Proxy::add_proxy(
					RuntimeOrigin::signed(delegator),
					delegate,
					ProxyType::Any,
					Some(sponsor),
				));

				assert!(!Proxy::has_sponsorship_approval(&delegator, &sponsor));
			});
		}

		#[test]
		fn should_be_authorized() {
			new_test_ext().execute_with(|| {
				let delegator = 1;
				let delegate = 2;
				let sponsor = 3;

				make_free_balance_be(&sponsor, 10);

				assert_noop!(
					Proxy::add_proxy(
						RuntimeOrigin::signed(delegator),
						delegate,
						ProxyType::Any,
						Some(sponsor),
					),
					Error::<Test>::SponsorshipUnauthorized
				);
			});
		}

		#[test]
		fn sponsor_can_register_agent() {
			new_test_ext().execute_with(|| {
				let sponsor = 3;
				let sponsor_agent = 4;

				make_free_balance_be(&sponsor, 10);

				assert_ok!(Proxy::register_sponsor_agent(
					RuntimeOrigin::signed(sponsor),
					sponsor_agent
				));

				assert!(Proxy::has_sponsor_agent(&sponsor, &sponsor_agent));

				System::assert_last_event(
					Event::SponsorAgentRegistered { sponsor, agent: sponsor_agent }.into(),
				);
			});
		}

		#[test]
		fn agent_can_be_registered_only_once() {
			new_test_ext().execute_with(|| {
				let sponsor = 3;
				let sponsor_agent = 4;
				let another_sponsor = 5;

				assert_ok!(Proxy::register_sponsor_agent(
					RuntimeOrigin::signed(sponsor),
					sponsor_agent
				));

				assert_noop!(
					Proxy::register_sponsor_agent(
						RuntimeOrigin::signed(another_sponsor),
						sponsor_agent
					),
					Error::<Test>::SponsorAgentAlreadyRegistered
				);
			});
		}

		#[test]
		fn sponsor_can_revoke_agent() {
			new_test_ext().execute_with(|| {
				let sponsor = 3;
				let sponsor_agent = 4;

				assert_ok!(Proxy::register_sponsor_agent(
					RuntimeOrigin::signed(sponsor),
					sponsor_agent
				));

				assert!(Proxy::has_sponsor_agent(&sponsor, &sponsor_agent));

				assert_ok!(Proxy::revoke_sponsor_agent(
					RuntimeOrigin::signed(sponsor),
					sponsor_agent
				));

				assert!(!Proxy::has_sponsor_agent(&sponsor, &sponsor_agent));

				System::assert_last_event(
					Event::SponsorAgentRevoked { sponsor, agent: sponsor_agent }.into(),
				);
			});
		}

		#[test]
		fn agent_can_approve_add_proxy() {
			new_test_ext().execute_with(|| {
				let delegator = 1;
				let sponsor = 3;
				let sponsor_agent = 4;

				assert_ok!(Proxy::register_sponsor_agent(
					RuntimeOrigin::signed(sponsor),
					sponsor_agent
				));

				assert_ok!(Proxy::approve_proxy_funding(
					RuntimeOrigin::signed(sponsor_agent),
					sponsor,
					delegator,
				));

				System::assert_last_event(
					Event::ProxySponsorshipApproved { delegator, sponsor, approver: sponsor_agent }
						.into(),
				);

				assert!(Proxy::has_sponsorship_approval(&delegator, &sponsor));
			});
		}

		#[test]
		fn sponsor_can_give_approve_direcly() {
			new_test_ext().execute_with(|| {
				let delegator = 1;
				let sponsor = 3;

				make_free_balance_be(&sponsor, 10);

				assert_ok!(Proxy::approve_proxy_funding(
					RuntimeOrigin::signed(sponsor),
					sponsor,
					delegator,
				));

				System::assert_last_event(
					Event::ProxySponsorshipApproved { delegator, sponsor, approver: sponsor }
						.into(),
				);

				assert!(Proxy::has_sponsorship_approval(&delegator, &sponsor));
			});
		}

		#[test]
		fn agent_cant_approve_add_proxy_if_not_registered() {
			new_test_ext().execute_with(|| {
				let delegator = 1;
				let sponsor = 3;
				let sponsor_agent = 4;

				make_free_balance_be(&sponsor, 10);

				assert_noop!(
					Proxy::approve_proxy_funding(
						RuntimeOrigin::signed(sponsor_agent),
						sponsor,
						delegator,
					),
					Error::<Test>::SponsorAgentUnauthorized
				);
			});
		}

		#[test]
		fn agent_revokation_should_invalidate_approvals() {
			new_test_ext().execute_with(|| {
				let delegator = 1;
				let sponsor = 3;
				let sponsor_agent = 4;

				make_free_balance_be(&sponsor, 10);

				assert_ok!(Proxy::register_sponsor_agent(
					RuntimeOrigin::signed(sponsor),
					sponsor_agent
				));

				assert_ok!(Proxy::approve_proxy_funding(
					RuntimeOrigin::signed(sponsor_agent),
					sponsor,
					delegator,
				));

				assert_ok!(Proxy::revoke_sponsor_agent(
					RuntimeOrigin::signed(sponsor),
					sponsor_agent
				));

				assert_noop!(
					Proxy::add_proxy(
						RuntimeOrigin::signed(delegator),
						2,
						ProxyType::Any,
						Some(sponsor),
					),
					Error::<Test>::SponsorshipUnauthorized
				);
			});
		}

		#[test]
		fn removing_proxy_should_free_sponsored_deposit() {
			new_test_ext().execute_with(|| {
				let delegator = 1;
				let delegate = 2;
				let sponsor = 3;
				let sponsor_agent = 4;

				make_free_balance_be(&sponsor, 10);

				assert_ok!(Proxy::register_sponsor_agent(
					RuntimeOrigin::signed(sponsor),
					sponsor_agent
				));

				assert_ok!(Proxy::approve_proxy_funding(
					RuntimeOrigin::signed(sponsor_agent),
					sponsor,
					delegator,
				));

				assert_ok!(Proxy::add_proxy(
					RuntimeOrigin::signed(delegator),
					delegate,
					ProxyType::Any,
					Some(sponsor),
				));

				assert_eq!(Balances::reserved_balance(sponsor), 1);

				assert_ok!(Proxy::remove_proxy(RuntimeOrigin::signed(delegator), delegate,));

				assert_eq!(Balances::reserved_balance(sponsor), 0);
			});
		}

		#[test]
		fn sponsor_can_remove_proxy() {
			new_test_ext().execute_with(|| {
				let delegator = 1;
				let delegate = 2;
				let sponsor = 3;
				let sponsor_agent = 4;

				make_free_balance_be(&sponsor, 10);

				assert_ok!(Proxy::register_sponsor_agent(
					RuntimeOrigin::signed(sponsor),
					sponsor_agent
				));

				assert_ok!(Proxy::approve_proxy_funding(
					RuntimeOrigin::signed(sponsor_agent),
					sponsor,
					delegator,
				));

				assert_ok!(Proxy::add_proxy(
					RuntimeOrigin::signed(delegator),
					delegate,
					ProxyType::Any,
					Some(sponsor),
				));

				assert_eq!(Balances::reserved_balance(sponsor), 1);

				assert_ok!(Proxy::remove_sponsored_proxy(
					RuntimeOrigin::signed(sponsor),
					delegator,
					delegate,
				));

				assert_eq!(Balances::reserved_balance(sponsor), 0);

				System::assert_last_event(
					Event::ProxyRemoved { delegator, delegate, removed_by_sponsor: Some(sponsor) }
						.into(),
				);
			});
		}

		#[test]
		fn invalidated_approvals_should_be_removed_on_idle() {
			new_test_ext().execute_with(|| {
				let delegator = 1;
				let sponsor = 3;
				let sponsor_agent = 4;

				assert_ok!(Proxy::register_sponsor_agent(
					RuntimeOrigin::signed(sponsor),
					sponsor_agent
				));

				assert_ok!(Proxy::approve_proxy_funding(
					RuntimeOrigin::signed(sponsor_agent),
					sponsor,
					delegator,
				));

				assert_ok!(Proxy::revoke_sponsor_agent(
					RuntimeOrigin::signed(sponsor),
					sponsor_agent
				));

				Proxy::on_idle(1, Weight::MAX);

				assert!(!SponsorshipApprovals::<Test>::contains_key((delegator, sponsor)));
			});
		}
	}

	mod approvals_cleanup {
		use super::*;

		#[test]
		fn should_remove_approvals_from_removed_agent() {
			new_test_ext().execute_with(|| {
				let delegator = 1;
				let sponsor = 3;
				let sponsor_agent = 4;

				assert_ok!(Proxy::register_sponsor_agent(
					RuntimeOrigin::signed(sponsor),
					sponsor_agent
				));

				assert_ok!(Proxy::approve_proxy_funding(
					RuntimeOrigin::signed(sponsor_agent),
					sponsor,
					delegator,
				));

				assert_ok!(Proxy::revoke_sponsor_agent(
					RuntimeOrigin::signed(sponsor),
					sponsor_agent
				));

				assert!(Proxy::cleanup_approvals());
				assert!(!SponsorshipApprovals::<Test>::contains_key((delegator, sponsor)));

				assert!(!Proxy::cleanup_approvals());
				assert!(!InvalidatedAgents::<Test>::contains_key(sponsor_agent));
			});
		}
	}
}
