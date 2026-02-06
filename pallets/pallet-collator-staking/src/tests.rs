// Copyright (C) BlockDeep Labs UG.
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use crate as collator_staking;
use crate::{
	mock::*, AutoCompound, BalanceOf, CandidacyBondRelease, CandidacyBondReleaseReason,
	CandidacyBondReleases, CandidateInfo, CandidateStake, CandidateStakeInfo, Candidates,
	ClaimableRewards, CollatorRewardPercentage, Config, CurrentSession, DesiredCandidates, Error,
	Event, ExtraReward, FreezeReason, IdentityCollator, Invulnerables, LastAuthoredBlock,
	MinCandidacyBond, MinStake, PerSessionRewards, ProducedBlocks, ReleaseQueues, ReleaseRequest,
	SessionInfo, SessionRemovedCandidates, StakeTarget, StakingPotAccountId, TotalBlocks,
	UserStake, UserStakeInfo,
};
use frame_support::pallet_prelude::TypedGet;
use frame_support::{
	assert_noop, assert_ok,
	traits::{
		fungible::{Inspect, InspectFreeze, Mutate},
		tokens::Preservation::Preserve,
		OnInitialize,
	},
};
use sp_runtime::{
	testing::UintAuthorityId,
	traits::{BadOrigin, Convert},
	BuildStorage, Percent, TokenError,
};
use std::ops::RangeInclusive;

type AccountId = <Test as frame_system::Config>::AccountId;

macro_rules! bbtreeset {
    ( $( $x:expr ),* ) => {
        {
			#[allow(unused_mut)]
            let mut set = sp_std::collections::btree_set::BTreeSet::new();
            $(
                set.insert($x);
            )*
            set.try_into().expect("Failed to create BTreeSet")
        }
    };
}

fn fund_account(acc: AccountId) {
	Balances::mint_into(&acc, 100).unwrap();
}

fn register_keys(acc: AccountId) {
	let key = MockSessionKeys { aura: UintAuthorityId(acc) };
	Session::set_keys(RuntimeOrigin::signed(acc), key, Vec::new()).unwrap();
}

fn register_candidates(range: RangeInclusive<AccountId>) {
	for ii in range {
		if ii > 5 {
			// only keys were registered in mock for 1 to 5
			fund_account(ii);
			register_keys(ii);
		}
		assert_ok!(CollatorStaking::register_as_candidate(
			RuntimeOrigin::signed(ii),
			MinCandidacyBond::<Test>::get()
		));
		System::assert_last_event(RuntimeEvent::CollatorStaking(Event::CandidateAdded {
			account: ii,
			deposit: MinCandidacyBond::<Test>::get(),
		}));
	}
}

fn candidate_list() -> Vec<(AccountId, CandidateInfo<BalanceOf<Test>>)> {
	let mut all_candidates = Candidates::<Test>::iter().collect::<Vec<_>>();
	all_candidates.sort_by_key(|(_, info)| info.stake);
	all_candidates
}

fn lock_for_staking(range: RangeInclusive<AccountId>) {
	for ii in range {
		let balance = CollatorStaking::get_free_balance(&ii);
		assert_ok!(CollatorStaking::lock(RuntimeOrigin::signed(ii), balance));
		System::assert_last_event(RuntimeEvent::CollatorStaking(Event::LockExtended {
			account: ii,
			amount: balance,
		}));
	}
}

mod extra {
	use super::*;

	#[test]
	fn identify_collator_should_work() {
		assert_eq!(IdentityCollator::convert(4), Some(4));
	}

	#[test]
	fn staking_pot_should_work() {
		assert_eq!(StakingPotAccountId::<Test>::get(), CollatorStaking::account_id());
	}
}

mod set_invulnerables {
	use super::*;

	#[test]
	fn it_should_set_invulnerables() {
		new_test_ext().execute_with(|| {
			initialize_to_block(1);
			let new_set = vec![1, 4, 3, 2];
			assert_ok!(CollatorStaking::set_invulnerables(
				RuntimeOrigin::signed(RootAccount::get()),
				new_set.clone()
			));
			System::assert_last_event(RuntimeEvent::CollatorStaking(Event::NewInvulnerables {
				invulnerables: vec![1, 2, 3, 4],
			}));
			assert_eq!(Invulnerables::<Test>::get(), vec![1, 2, 3, 4]);

			// cannot set with non-root.
			assert_noop!(
				CollatorStaking::set_invulnerables(RuntimeOrigin::signed(1), new_set),
				BadOrigin
			);
		});
	}

	#[test]
	fn cannot_empty_invulnerables_if_not_enough_candidates() {
		new_test_ext().execute_with(|| {
			initialize_to_block(1);

			assert_noop!(
				CollatorStaking::set_invulnerables(
					RuntimeOrigin::signed(RootAccount::get()),
					vec![]
				),
				Error::<Test>::TooFewEligibleCollators
			);
		});
	}

	#[test]
	fn it_should_set_invulnerables_even_with_some_invalid() {
		new_test_ext().execute_with(|| {
			initialize_to_block(1);

			assert_eq!(Invulnerables::<Test>::get(), vec![1, 2]);
			let new_with_invalid = vec![1, 4, 3, 42, 2, 1000];

			assert_ok!(CollatorStaking::set_invulnerables(
				RuntimeOrigin::signed(RootAccount::get()),
				new_with_invalid
			));
			System::assert_has_event(RuntimeEvent::CollatorStaking(
				Event::InvalidInvulnerableSkipped { account: 42 },
			));
			System::assert_last_event(RuntimeEvent::CollatorStaking(Event::NewInvulnerables {
				invulnerables: vec![1, 2, 3, 4],
			}));

			// should succeed and order them, but not include 42
			assert_eq!(Invulnerables::<Test>::get(), vec![1, 2, 3, 4]);
		});
	}

	#[test]
	fn it_should_not_allow_duplicated_invulnerables() {
		new_test_ext().execute_with(|| {
			initialize_to_block(1);

			assert_eq!(Invulnerables::<Test>::get(), vec![1, 2]);
			let new_with_duplicated = vec![1, 1, 2, 4, 3, 2];

			assert_noop!(
				CollatorStaking::set_invulnerables(
					RuntimeOrigin::signed(RootAccount::get()),
					new_with_duplicated
				),
				Error::<Test>::DuplicatedInvulnerables
			);
		});
	}

	#[test]
	fn it_should_not_allow_too_many_invalid_invulnerables() {
		new_test_ext().execute_with(|| {
			initialize_to_block(1);

			assert_eq!(Invulnerables::<Test>::get(), vec![1, 2]);
			let new_with_many_invalid = vec![1000, 1001, 1002, 1003, 1004, 1005, 1006];

			assert_noop!(
				CollatorStaking::set_invulnerables(
					RuntimeOrigin::signed(RootAccount::get()),
					new_with_many_invalid
				),
				Error::<Test>::TooFewEligibleCollators
			);
		});
	}

	#[test]
	fn should_not_allow_to_set_invulnerables_if_already_candidates() {
		new_test_ext().execute_with(|| {
			initialize_to_block(1);

			assert_eq!(Candidates::<Test>::count(), 0);
			register_candidates(3..=4);
			assert_eq!(Balances::balance_frozen(&FreezeReason::CandidacyBond.into(), &3), 10);
			assert_eq!(Balances::balance_frozen(&FreezeReason::CandidacyBond.into(), &4), 10);
			assert_noop!(
				CollatorStaking::set_invulnerables(
					RuntimeOrigin::signed(RootAccount::get()),
					vec![1, 2, 3]
				),
				Error::<Test>::AlreadyCandidate
			);
		});
	}
}

mod set_desired_candidates {
	use super::*;

	#[test]
	fn set_desired_candidates_works() {
		new_test_ext().execute_with(|| {
			initialize_to_block(1);

			// given
			assert_eq!(DesiredCandidates::<Test>::get(), 2);

			// can set
			assert_ok!(CollatorStaking::set_desired_candidates(
				RuntimeOrigin::signed(RootAccount::get()),
				4
			));
			System::assert_last_event(RuntimeEvent::CollatorStaking(Event::NewDesiredCandidates {
				desired_candidates: 4,
			}));
			assert_eq!(DesiredCandidates::<Test>::get(), 4);

			// rejects bad origin
			assert_noop!(
				CollatorStaking::set_desired_candidates(RuntimeOrigin::signed(1), 2),
				BadOrigin
			);
			// rejects too many
			assert_noop!(
				CollatorStaking::set_desired_candidates(
					RuntimeOrigin::signed(RootAccount::get()),
					50
				),
				Error::<Test>::TooManyDesiredCandidates
			);
		});
	}

	#[test]
	fn cannot_set_desired_candidates_if_under_min_collator_limit() {
		new_test_ext().execute_with(|| {
			initialize_to_block(1);

			// given
			assert_eq!(DesiredCandidates::<Test>::get(), 2);
			assert_eq!(<Test as Config>::MinEligibleCollators::get(), 1);
			register_candidates(3..=3);

			assert_ok!(CollatorStaking::set_invulnerables(
				RuntimeOrigin::signed(RootAccount::get()),
				vec![]
			));
			assert_noop!(
				CollatorStaking::set_desired_candidates(
					RuntimeOrigin::signed(RootAccount::get()),
					0
				),
				Error::<Test>::TooFewEligibleCollators
			);
		});
	}
}

mod add_invulnerable {
	use super::*;

	#[test]
	fn add_invulnerable_works() {
		new_test_ext().execute_with(|| {
			initialize_to_block(1);

			assert_eq!(Invulnerables::<Test>::get(), vec![1, 2]);
			let new = 3;

			// function runs
			assert_ok!(CollatorStaking::add_invulnerable(
				RuntimeOrigin::signed(RootAccount::get()),
				new
			));

			System::assert_last_event(RuntimeEvent::CollatorStaking(Event::InvulnerableAdded {
				account: new,
			}));

			// same element cannot be added more than once
			assert_noop!(
				CollatorStaking::add_invulnerable(RuntimeOrigin::signed(RootAccount::get()), new),
				Error::<Test>::AlreadyInvulnerable
			);

			// new element is now part of the invulnerables list
			assert!(Invulnerables::<Test>::get().to_vec().contains(&new));

			// cannot add with non-root
			assert_noop!(
				CollatorStaking::add_invulnerable(RuntimeOrigin::signed(1), new),
				BadOrigin
			);

			// cannot add invulnerable without associated validator keys
			let not_validator = 42;
			assert_noop!(
				CollatorStaking::add_invulnerable(
					RuntimeOrigin::signed(RootAccount::get()),
					not_validator
				),
				Error::<Test>::CollatorNotRegistered
			);
		});
	}

	#[test]
	fn invulnerable_limit_works() {
		new_test_ext().execute_with(|| {
			initialize_to_block(1);

			assert_eq!(Invulnerables::<Test>::get(), vec![1, 2]);

			// MaxInvulnerables: u32 = 20
			for ii in 3..=21 {
				// only keys were registered in mock for 1 to 5
				if ii > 5 {
					Balances::mint_into(&ii, 100).unwrap();
					let key = MockSessionKeys { aura: UintAuthorityId(ii) };
					Session::set_keys(RuntimeOrigin::signed(ii), key, Vec::new()).unwrap();
				}
				assert_eq!(Balances::balance(&ii), 100);
				if ii < 21 {
					assert_ok!(CollatorStaking::add_invulnerable(
						RuntimeOrigin::signed(RootAccount::get()),
						ii
					));
					System::assert_last_event(RuntimeEvent::CollatorStaking(
						Event::InvulnerableAdded { account: ii },
					));
				} else {
					assert_noop!(
						CollatorStaking::add_invulnerable(
							RuntimeOrigin::signed(RootAccount::get()),
							ii
						),
						Error::<Test>::TooManyInvulnerables
					);
				}
			}
			let expected: Vec<u64> = (1..=20).collect();
			assert_eq!(Invulnerables::<Test>::get(), expected);

			// Cannot set too many Invulnerables
			let too_many_invulnerables: Vec<u64> = (1..=21).collect();
			assert_noop!(
				CollatorStaking::set_invulnerables(
					RuntimeOrigin::signed(RootAccount::get()),
					too_many_invulnerables
				),
				Error::<Test>::TooManyInvulnerables
			);
			assert_eq!(Invulnerables::<Test>::get(), expected);
		});
	}

	#[test]
	fn candidate_to_invulnerable_should_fail() {
		new_test_ext().execute_with(|| {
			initialize_to_block(1);
			assert_eq!(DesiredCandidates::<Test>::get(), 2);
			assert_eq!(MinCandidacyBond::<Test>::get(), 10);

			assert_eq!(Candidates::<Test>::count(), 0);
			assert_eq!(Invulnerables::<Test>::get(), vec![1, 2]);

			assert_eq!(Balances::balance(&3), 100);
			assert_eq!(Balances::balance(&4), 100);

			register_candidates(3..=4);

			assert_eq!(
				CandidateStake::<Test>::get(3, 3),
				CandidateStakeInfo { stake: 0, session: 0 }
			);
			assert_eq!(Balances::balance_frozen(&FreezeReason::CandidacyBond.into(), &3), 10);
			assert_eq!(
				CandidateStake::<Test>::get(4, 4),
				CandidateStakeInfo { stake: 0, session: 0 }
			);
			assert_eq!(Balances::balance_frozen(&FreezeReason::CandidacyBond.into(), &4), 10);

			lock_for_staking(3..=5);
			assert_ok!(CollatorStaking::stake(
				RuntimeOrigin::signed(5),
				vec![StakeTarget { candidate: 3, stake: 10 }].try_into().unwrap()
			));
			assert_ok!(CollatorStaking::stake(
				RuntimeOrigin::signed(3),
				vec![StakeTarget { candidate: 3, stake: 10 }].try_into().unwrap()
			));
			assert_ok!(CollatorStaking::stake(
				RuntimeOrigin::signed(4),
				vec![StakeTarget { candidate: 4, stake: 10 }].try_into().unwrap()
			));

			assert_noop!(
				CollatorStaking::add_invulnerable(RuntimeOrigin::signed(RootAccount::get()), 3),
				Error::<Test>::AlreadyCandidate
			);
		});
	}
}

mod remove_invulnerable {
	use super::*;

	#[test]
	fn remove_invulnerable_works() {
		new_test_ext().execute_with(|| {
			initialize_to_block(1);

			assert_eq!(Invulnerables::<Test>::get(), vec![1, 2]);

			assert_ok!(CollatorStaking::add_invulnerable(
				RuntimeOrigin::signed(RootAccount::get()),
				4
			));
			System::assert_last_event(RuntimeEvent::CollatorStaking(Event::InvulnerableAdded {
				account: 4,
			}));
			assert_ok!(CollatorStaking::add_invulnerable(
				RuntimeOrigin::signed(RootAccount::get()),
				3
			));
			System::assert_last_event(RuntimeEvent::CollatorStaking(Event::InvulnerableAdded {
				account: 3,
			}));

			assert_eq!(Invulnerables::<Test>::get(), vec![1, 2, 3, 4]);

			assert_ok!(CollatorStaking::remove_invulnerable(
				RuntimeOrigin::signed(RootAccount::get()),
				2
			));
			System::assert_last_event(RuntimeEvent::CollatorStaking(Event::InvulnerableRemoved {
				account_id: 2,
			}));
			assert_eq!(Invulnerables::<Test>::get(), vec![1, 3, 4]);

			// cannot remove invulnerable not in the list
			assert_noop!(
				CollatorStaking::remove_invulnerable(RuntimeOrigin::signed(RootAccount::get()), 2),
				Error::<Test>::NotInvulnerable
			);

			// cannot remove without privilege
			assert_noop!(
				CollatorStaking::remove_invulnerable(RuntimeOrigin::signed(1), 3),
				BadOrigin
			);
		});
	}
}

mod set_min_candidacy_bond {
	use super::*;

	#[test]
	fn set_candidacy_bond_empty_candidate_list() {
		new_test_ext().execute_with(|| {
			initialize_to_block(1);

			// given
			assert_eq!(MinCandidacyBond::<Test>::get(), 10);
			assert_eq!(Candidates::<Test>::count(), 0);

			// can decrease without candidates
			assert_ok!(CollatorStaking::set_min_candidacy_bond(
				RuntimeOrigin::signed(RootAccount::get()),
				7
			));
			System::assert_last_event(RuntimeEvent::CollatorStaking(Event::NewMinCandidacyBond {
				bond_amount: 7,
			}));
			assert_eq!(MinCandidacyBond::<Test>::get(), 7);
			assert_eq!(Candidates::<Test>::count(), 0);

			// rejects bad origin.
			assert_noop!(
				CollatorStaking::set_min_candidacy_bond(RuntimeOrigin::signed(1), 8),
				BadOrigin
			);

			// can increase without candidates
			assert_ok!(CollatorStaking::set_min_candidacy_bond(
				RuntimeOrigin::signed(RootAccount::get()),
				20
			));
			System::assert_last_event(RuntimeEvent::CollatorStaking(Event::NewMinCandidacyBond {
				bond_amount: 20,
			}));
			assert_eq!(Candidates::<Test>::count(), 0);
			assert_eq!(MinCandidacyBond::<Test>::get(), 20);
		});
	}

	#[test]
	fn set_candidacy_bond_with_one_candidate() {
		new_test_ext().execute_with(|| {
			initialize_to_block(1);

			// given
			assert_eq!(MinCandidacyBond::<Test>::get(), 10);
			assert_eq!(Candidates::<Test>::count(), 0);

			let candidate_3 = CandidateInfo { stake: 0, stakers: 0 };

			register_candidates(3..=3);
			assert_eq!(candidate_list(), vec![(3, candidate_3.clone())]);
			assert_eq!(
				CandidateStake::<Test>::get(3, 3),
				CandidateStakeInfo { stake: 0, session: 0 }
			);

			// can decrease with one candidate
			assert_ok!(CollatorStaking::set_min_candidacy_bond(
				RuntimeOrigin::signed(RootAccount::get()),
				7
			));
			System::assert_last_event(RuntimeEvent::CollatorStaking(Event::NewMinCandidacyBond {
				bond_amount: 7,
			}));
			assert_eq!(MinCandidacyBond::<Test>::get(), 7);
			initialize_to_block(10);
			assert_eq!(candidate_list(), vec![(3, candidate_3.clone())]);

			// can increase up to initial deposit
			assert_ok!(CollatorStaking::set_min_candidacy_bond(
				RuntimeOrigin::signed(RootAccount::get()),
				10
			));
			System::assert_last_event(RuntimeEvent::CollatorStaking(Event::NewMinCandidacyBond {
				bond_amount: 10,
			}));
			assert_eq!(MinCandidacyBond::<Test>::get(), 10);
			initialize_to_block(20);
			assert_eq!(candidate_list(), vec![(3, candidate_3.clone())]);

			// can increase past initial deposit, kicking candidates under the new value
			assert_ok!(CollatorStaking::set_min_candidacy_bond(
				RuntimeOrigin::signed(RootAccount::get()),
				20
			));
			System::assert_last_event(RuntimeEvent::CollatorStaking(Event::NewMinCandidacyBond {
				bond_amount: 20,
			}));
			assert_eq!(MinCandidacyBond::<Test>::get(), 20);
			initialize_to_block(30);
			assert_eq!(candidate_list(), vec![]);
		});
	}

	#[test]
	fn set_candidacy_bond_with_many_candidates_same_deposit() {
		new_test_ext().execute_with(|| {
			initialize_to_block(1);

			// given
			assert_eq!(MinCandidacyBond::<Test>::get(), 10);
			assert_eq!(Candidates::<Test>::count(), 0);

			let candidate_3 = CandidateInfo { stake: 0, stakers: 0 };
			let candidate_4 = CandidateInfo { stake: 0, stakers: 0 };
			let candidate_5 = CandidateInfo { stake: 0, stakers: 0 };

			register_candidates(3..=5);
			assert_eq!(
				candidate_list(),
				vec![(5, candidate_5.clone()), (3, candidate_3.clone()), (4, candidate_4.clone())]
			);

			// can decrease with multiple candidates
			assert_ok!(CollatorStaking::set_min_candidacy_bond(
				RuntimeOrigin::signed(RootAccount::get()),
				2
			));
			System::assert_last_event(RuntimeEvent::CollatorStaking(Event::NewMinCandidacyBond {
				bond_amount: 2,
			}));
			assert_eq!(MinCandidacyBond::<Test>::get(), 2);
			CollatorStaking::kick_stale_candidates();
			assert_eq!(
				candidate_list(),
				vec![(5, candidate_5.clone()), (3, candidate_3.clone()), (4, candidate_4.clone())]
			);

			// can increase up to initial deposit
			assert_ok!(CollatorStaking::set_min_candidacy_bond(
				RuntimeOrigin::signed(RootAccount::get()),
				10
			));
			System::assert_last_event(RuntimeEvent::CollatorStaking(Event::NewMinCandidacyBond {
				bond_amount: 10,
			}));
			assert_eq!(MinCandidacyBond::<Test>::get(), 10);
			CollatorStaking::kick_stale_candidates();
			assert_eq!(
				candidate_list(),
				vec![(5, candidate_5.clone()), (3, candidate_3.clone()), (4, candidate_4.clone())]
			);

			// can increase past initial deposit
			assert_ok!(CollatorStaking::set_min_candidacy_bond(
				RuntimeOrigin::signed(RootAccount::get()),
				20
			));
			System::assert_last_event(RuntimeEvent::CollatorStaking(Event::NewMinCandidacyBond {
				bond_amount: 20,
			}));
			assert_eq!(MinCandidacyBond::<Test>::get(), 20);
			assert_ok!(CollatorStaking::update_candidacy_bond(RuntimeOrigin::signed(5), 20));
			System::assert_last_event(RuntimeEvent::CollatorStaking(
				Event::<Test>::CandidacyBondUpdated { candidate: 5, new_bond: 20 },
			));
			CollatorStaking::kick_stale_candidates();
			assert_eq!(candidate_list(), vec![(5, candidate_5)]);
			System::assert_has_event(RuntimeEvent::CollatorStaking(
				Event::<Test>::CandidateRemoved { account: 3 },
			));
			System::assert_has_event(RuntimeEvent::CollatorStaking(
				Event::<Test>::CandidateRemoved { account: 4 },
			));
		});
	}
}

mod register_as_candidate {
	use super::*;

	#[test]
	fn cannot_register_candidate_if_too_many() {
		new_test_ext().execute_with(|| {
			initialize_to_block(1);

			// MaxCandidates: u32 = 20
			assert_eq!(<Test as Config>::MaxCandidates::get(), 20);

			// Aside from 3, 4, and 5, create enough accounts to have 21 potential
			// candidates.
			for acc in 3..=22 {
				fund_account(acc);
				register_keys(acc);
				let bond = if acc > 3 { 20 } else { 10 };
				assert_ok!(CollatorStaking::register_as_candidate(
					RuntimeOrigin::signed(acc),
					bond
				));
			}
			assert_eq!(Candidates::<Test>::count(), 20);
			fund_account(23);
			register_keys(23);
			assert_ok!(CollatorStaking::register_as_candidate(RuntimeOrigin::signed(23), 11));
			assert!(Candidates::<Test>::get(23).is_some());
			// Account 6 had only 10 as candidacy bond, not 20 like the rest.
			assert!(Candidates::<Test>::get(3).is_none());
		})
	}

	#[test]
	fn cannot_register_as_candidate_if_invulnerable() {
		new_test_ext().execute_with(|| {
			initialize_to_block(1);

			// given
			assert_eq!(Invulnerables::<Test>::get(), vec![1, 2]);

			// can't 1 because it is invulnerable.
			assert_noop!(
				CollatorStaking::register_as_candidate(
					RuntimeOrigin::signed(1),
					MinCandidacyBond::<Test>::get()
				),
				Error::<Test>::AlreadyInvulnerable,
			);
		})
	}

	#[test]
	fn cannot_register_as_candidate_if_bond_too_low() {
		new_test_ext().execute_with(|| {
			initialize_to_block(1);

			assert_noop!(
				CollatorStaking::register_as_candidate(RuntimeOrigin::signed(3), 1),
				Error::<Test>::InvalidCandidacyBond,
			);
		})
	}

	#[test]
	fn cannot_register_as_candidate_if_keys_not_registered() {
		new_test_ext().execute_with(|| {
			initialize_to_block(1);

			// can't 42 because keys not registered.
			assert_noop!(
				CollatorStaking::register_as_candidate(
					RuntimeOrigin::signed(42),
					MinCandidacyBond::<Test>::get()
				),
				Error::<Test>::CollatorNotRegistered
			);
		})
	}

	#[test]
	fn cannot_register_dupe_candidate() {
		new_test_ext().execute_with(|| {
			initialize_to_block(1);

			// can add 3 as candidate
			register_candidates(3..=3);
			let addition = CandidateInfo { stake: 0, stakers: 0 };
			assert_eq!(candidate_list(), vec![(3, addition)]);
			assert_eq!(LastAuthoredBlock::<Test>::get(3), 11);
			assert_eq!(Balances::balance_frozen(&FreezeReason::CandidacyBond.into(), &3), 10);
			assert_eq!(
				CandidateStake::<Test>::get(3, 3),
				CandidateStakeInfo { stake: 0, session: 0 }
			);

			// but no more
			assert_noop!(
				CollatorStaking::register_as_candidate(
					RuntimeOrigin::signed(3),
					MinCandidacyBond::<Test>::get()
				),
				Error::<Test>::AlreadyCandidate,
			);
		})
	}

	#[test]
	fn cannot_register_as_candidate_if_poor() {
		new_test_ext().execute_with(|| {
			initialize_to_block(1);

			assert_eq!(Balances::balance(&3), 100);
			assert_eq!(Balances::balance(&33), 0);

			// works
			register_candidates(3..=3);

			// poor
			assert_noop!(
				CollatorStaking::register_as_candidate(
					RuntimeOrigin::signed(33),
					MinCandidacyBond::<Test>::get()
				),
				Error::<Test>::InsufficientFreeBalance,
			);
		});
	}

	#[test]
	fn register_as_candidate_works() {
		new_test_ext().execute_with(|| {
			initialize_to_block(1);

			// given
			assert_eq!(DesiredCandidates::<Test>::get(), 2);
			assert_eq!(MinCandidacyBond::<Test>::get(), 10);

			assert_eq!(Candidates::<Test>::count(), 0);
			assert_eq!(Invulnerables::<Test>::get(), vec![1, 2]);

			// take two endowed, non-invulnerables accounts.
			assert_eq!(Balances::balance(&3), 100);
			assert_eq!(
				CandidateStake::<Test>::get(3, 3),
				CandidateStakeInfo { stake: 0, session: 0 }
			);
			assert_eq!(Balances::balance(&4), 100);
			assert_eq!(
				CandidateStake::<Test>::get(4, 4),
				CandidateStakeInfo { stake: 0, session: 0 }
			);

			register_candidates(3..=4);

			assert_eq!(Balances::balance_frozen(&FreezeReason::CandidacyBond.into(), &3), 10);
			assert_eq!(
				CandidateStake::<Test>::get(3, 3),
				CandidateStakeInfo { stake: 0, session: 0 }
			);
			assert_eq!(Balances::balance_frozen(&FreezeReason::CandidacyBond.into(), &4), 10);
			assert_eq!(
				CandidateStake::<Test>::get(4, 4),
				CandidateStakeInfo { stake: 0, session: 0 }
			);

			assert_eq!(Candidates::<Test>::count(), 2);
		});
	}

	#[test]
	fn register_as_candidate_counts_old_stake_when_rejoining() {
		new_test_ext().execute_with(|| {
			initialize_to_block(1);

			// given
			assert_eq!(DesiredCandidates::<Test>::get(), 2);
			assert_eq!(MinCandidacyBond::<Test>::get(), 10);
			assert_eq!(Candidates::<Test>::count(), 0);
			assert_eq!(Invulnerables::<Test>::get(), vec![1, 2]);

			// register the first time
			assert_eq!(Balances::balance(&3), 100);
			assert_eq!(
				CandidateStake::<Test>::get(3, 3),
				CandidateStakeInfo { stake: 0, session: 0 }
			);
			register_candidates(3..=3);
			assert_eq!(Balances::balance_frozen(&FreezeReason::CandidacyBond.into(), &3), 10);
			assert_eq!(
				CandidateStake::<Test>::get(3, 3),
				CandidateStakeInfo { stake: 0, session: 0 }
			);
			assert_eq!(Candidates::<Test>::count(), 1);
			assert_eq!(Candidates::<Test>::get(3), Some(CandidateInfo { stake: 0, stakers: 0 }));

			// another user adds stake
			fund_account(4);
			assert_ok!(CollatorStaking::lock(RuntimeOrigin::signed(4), 60));
			System::assert_last_event(RuntimeEvent::CollatorStaking(Event::LockExtended {
				account: 4,
				amount: 60,
			}));
			assert_ok!(CollatorStaking::stake(
				RuntimeOrigin::signed(4),
				vec![StakeTarget { candidate: 3, stake: 60 }].try_into().unwrap()
			));
			assert_eq!(
				CandidateStake::<Test>::get(3, 4),
				CandidateStakeInfo { stake: 60, session: 0 }
			);
			assert_eq!(Candidates::<Test>::get(3), Some(CandidateInfo { stake: 60, stakers: 1 }));

			// the candidate leaves
			assert_ok!(CollatorStaking::leave_intent(RuntimeOrigin::signed(3)));
			assert_eq!(
				SessionRemovedCandidates::<Test>::get(3),
				Some(CandidateInfo { stake: 60, stakers: 1 })
			);
			// the stake remains the same
			assert_eq!(
				CandidateStake::<Test>::get(3, 4),
				CandidateStakeInfo { stake: 60, session: 0 }
			);
			assert_eq!(Candidates::<Test>::count(), 0);

			// and finally rejoins and the stake should remain
			assert_ok!(CollatorStaking::register_as_candidate(
				RuntimeOrigin::signed(3),
				MinCandidacyBond::<Test>::get()
			));
			assert_eq!(
				CandidateStake::<Test>::get(3, 4),
				CandidateStakeInfo { stake: 60, session: 0 }
			);
			assert_eq!(Candidates::<Test>::count(), 1);
			assert_eq!(Candidates::<Test>::get(3), Some(CandidateInfo { stake: 60, stakers: 1 }));
		});
	}

	#[test]
	fn register_as_candidate_reuses_old_bond_if_replaced() {
		new_test_ext().execute_with(|| {
			initialize_to_block(1);

			// given
			assert_eq!(DesiredCandidates::<Test>::get(), 2);
			assert_eq!(MinCandidacyBond::<Test>::get(), 10);
			assert_eq!(Candidates::<Test>::count(), 0);
			assert_eq!(Invulnerables::<Test>::get(), vec![1, 2]);

			// register the first time
			assert_eq!(Balances::balance(&3), 100);
			assert_eq!(
				CandidateStake::<Test>::get(3, 3),
				CandidateStakeInfo { stake: 0, session: 0 }
			);
			register_candidates(3..=3);
			assert_eq!(Balances::balance_frozen(&FreezeReason::CandidacyBond.into(), &3), 10);
			assert_eq!(
				CandidateStake::<Test>::get(3, 3),
				CandidateStakeInfo { stake: 0, session: 0 }
			);
			assert_eq!(Candidates::<Test>::count(), 1);
			assert_eq!(Candidates::<Test>::get(3), Some(CandidateInfo { stake: 0, stakers: 0 }));
			assert_eq!(CollatorStaking::get_bond(&3), 10);

			// the candidate is replaced (artificially)
			assert_ok!(CollatorStaking::leave_intent(RuntimeOrigin::signed(3)));
			CandidacyBondReleases::<Test>::mutate(3, |maybe_bond_release| {
				let bond_release = maybe_bond_release.as_mut().unwrap();
				bond_release.reason = CandidacyBondReleaseReason::Replaced;
			});
			assert_eq!(CollatorStaking::get_releasing_balance(&3), 10);
			assert_eq!(CollatorStaking::get_bond(&3), 0);

			// and finally rejoins using the old candidacy bond
			assert_ok!(CollatorStaking::register_as_candidate(
				RuntimeOrigin::signed(3),
				MinCandidacyBond::<Test>::get()
			));
			assert_eq!(CollatorStaking::get_releasing_balance(&3), 0);
			assert_eq!(CollatorStaking::get_bond(&3), 10);
		});
	}

	#[test]
	fn register_as_candidate_does_not_reuse_old_bond_if_wrong_reason() {
		new_test_ext().execute_with(|| {
			initialize_to_block(1);

			// given
			assert_eq!(DesiredCandidates::<Test>::get(), 2);
			assert_eq!(MinCandidacyBond::<Test>::get(), 10);
			assert_eq!(Candidates::<Test>::count(), 0);
			assert_eq!(Invulnerables::<Test>::get(), vec![1, 2]);

			// register the first time
			assert_eq!(Balances::balance(&3), 100);
			assert_eq!(
				CandidateStake::<Test>::get(3, 3),
				CandidateStakeInfo { stake: 0, session: 0 }
			);
			register_candidates(3..=3);
			assert_eq!(Balances::balance_frozen(&FreezeReason::CandidacyBond.into(), &3), 10);
			assert_eq!(
				CandidateStake::<Test>::get(3, 3),
				CandidateStakeInfo { stake: 0, session: 0 }
			);
			assert_eq!(Candidates::<Test>::count(), 1);
			assert_eq!(Candidates::<Test>::get(3), Some(CandidateInfo { stake: 0, stakers: 0 }));
			assert_eq!(CollatorStaking::get_bond(&3), 10);

			// the candidate removes itself
			assert_ok!(CollatorStaking::leave_intent(RuntimeOrigin::signed(3)));
			assert_eq!(CollatorStaking::get_releasing_balance(&3), 10);
			assert_eq!(CollatorStaking::get_bond(&3), 0);

			// and finally rejoins using the old candidacy bond
			assert_ok!(CollatorStaking::register_as_candidate(
				RuntimeOrigin::signed(3),
				MinCandidacyBond::<Test>::get()
			));
			// the old locked candidacy bond should remain
			assert_eq!(CollatorStaking::get_releasing_balance(&3), 10);
			assert_eq!(CollatorStaking::get_bond(&3), 10);
		});
	}

	#[test]
	fn register_leave_register_leave_again() {
		new_test_ext().execute_with(|| {
			initialize_to_block(1);

			// First registration
			// Ensure preconditions
			assert_eq!(Balances::balance(&3), 100);
			assert_eq!(
				CandidateStake::<Test>::get(3, 3),
				CandidateStakeInfo { stake: 0, session: 0 }
			);

			register_candidates(3..=3);
			assert_eq!(Balances::balance_frozen(&FreezeReason::CandidacyBond.into(), &3), 10);
			assert_eq!(
				CandidateStake::<Test>::get(3, 3),
				CandidateStakeInfo { stake: 0, session: 0 }
			);
			assert_eq!(Candidates::<Test>::count(), 1);
			assert_eq!(Candidates::<Test>::get(3), Some(CandidateInfo { stake: 0, stakers: 0 }));
			assert_eq!(CollatorStaking::get_bond(&3), 10);
			assert_eq!(CandidacyBondReleases::<Test>::get(3), None);

			// First leave
			assert_ok!(CollatorStaking::leave_intent(RuntimeOrigin::signed(3)));
			assert_eq!(CollatorStaking::get_releasing_balance(&3), 10);
			assert_eq!(CollatorStaking::get_bond(&3), 0);
			assert_eq!(
				CandidacyBondReleases::<Test>::get(3),
				Some(CandidacyBondRelease {
					bond: 10,
					block: 6,
					reason: CandidacyBondReleaseReason::Left
				})
			);

			// Re-register
			assert_ok!(CollatorStaking::register_as_candidate(
				RuntimeOrigin::signed(3),
				MinCandidacyBond::<Test>::get()
			));
			assert_eq!(CollatorStaking::get_releasing_balance(&3), 10);
			assert_eq!(CollatorStaking::get_bond(&3), 10);
			assert_eq!(
				CandidacyBondReleases::<Test>::get(3),
				Some(CandidacyBondRelease {
					bond: 10,
					block: 6,
					reason: CandidacyBondReleaseReason::Left
				})
			);

			// Second leave. The bond should accumulate.
			assert_ok!(CollatorStaking::leave_intent(RuntimeOrigin::signed(3)));
			assert_eq!(CollatorStaking::get_releasing_balance(&3), 20);
			assert_eq!(CollatorStaking::get_bond(&3), 0);
			assert_eq!(
				CandidacyBondReleases::<Test>::get(3),
				Some(CandidacyBondRelease {
					bond: 20, // 10 the first time, and 10 the second
					block: 6,
					reason: CandidacyBondReleaseReason::Left
				})
			);
		});
	}
}

mod leave_intent {
	use super::*;

	#[test]
	fn cannot_unregister_candidate_if_too_few() {
		new_test_ext().execute_with(|| {
			initialize_to_block(1);

			assert_eq!(Candidates::<Test>::count(), 0);
			assert_eq!(Invulnerables::<Test>::get(), vec![1, 2]);
			assert_ok!(CollatorStaking::remove_invulnerable(
				RuntimeOrigin::signed(RootAccount::get()),
				1
			));
			System::assert_last_event(RuntimeEvent::CollatorStaking(Event::InvulnerableRemoved {
				account_id: 1,
			}));
			assert_noop!(
				CollatorStaking::remove_invulnerable(RuntimeOrigin::signed(RootAccount::get()), 2),
				Error::<Test>::TooFewEligibleCollators,
			);

			// reset desired candidates:
			DesiredCandidates::<Test>::set(1);
			register_candidates(4..=4);

			// now we can remove `2`
			assert_ok!(CollatorStaking::remove_invulnerable(
				RuntimeOrigin::signed(RootAccount::get()),
				2
			));
			System::assert_last_event(RuntimeEvent::CollatorStaking(Event::InvulnerableRemoved {
				account_id: 2,
			}));

			// can not remove too few
			assert_noop!(
				CollatorStaking::leave_intent(RuntimeOrigin::signed(4)),
				Error::<Test>::TooFewEligibleCollators,
			);
		})
	}

	#[test]
	fn leave_intent() {
		new_test_ext().execute_with(|| {
			initialize_to_block(1);

			// register a candidate.
			register_candidates(3..=3);
			assert_eq!(Balances::balance_frozen(&FreezeReason::CandidacyBond.into(), &3), 10);
			assert_eq!(
				CandidateStake::<Test>::get(3, 3),
				CandidateStakeInfo { stake: 0, session: 0 }
			);

			// register too so can leave above min candidates
			register_candidates(5..=5);
			assert_eq!(Balances::balance_frozen(&FreezeReason::CandidacyBond.into(), &5), 10);
			assert_eq!(
				CandidateStake::<Test>::get(5, 5),
				CandidateStakeInfo { stake: 0, session: 0 }
			);

			// cannot leave if not candidate.
			assert_noop!(
				CollatorStaking::leave_intent(RuntimeOrigin::signed(4)),
				Error::<Test>::NotCandidate
			);

			// Unstake request is created
			assert_eq!(ReleaseQueues::<Test>::get(3), vec![]);
			assert_eq!(Balances::balance_frozen(&FreezeReason::CandidacyBond.into(), &3), 10);

			assert_eq!(CandidacyBondReleases::<Test>::get(3), None);
			assert_ok!(CollatorStaking::leave_intent(RuntimeOrigin::signed(3)));
			assert_eq!(
				CandidacyBondReleases::<Test>::get(3),
				Some(CandidacyBondRelease {
					bond: 10,
					block: 6,
					reason: CandidacyBondReleaseReason::Left
				})
			);

			assert_eq!(Balances::balance_frozen(&FreezeReason::CandidacyBond.into(), &3), 0);
			assert_eq!(Balances::balance_frozen(&FreezeReason::Releasing.into(), &3), 10);
			assert_eq!(
				CandidateStake::<Test>::get(3, 3),
				CandidateStakeInfo { stake: 0, session: 0 }
			);
			assert_eq!(LastAuthoredBlock::<Test>::get(3), 0);
			assert_eq!(
				SessionRemovedCandidates::<Test>::get(3),
				Some(CandidateInfo { stake: 0, stakers: 0 })
			);
		});
	}

	#[test]
	fn leave_with_release_queue_full_should_work() {
		new_test_ext().execute_with(|| {
			initialize_to_block(1);

			register_candidates(3..=3);

			assert_eq!(ReleaseQueues::<Test>::get(3), vec![]);
			let release_queue_max_len = <Test as Config>::MaxStakedCandidates::get();
			let lock = (release_queue_max_len * 2) as u64;
			assert_ok!(CollatorStaking::lock(RuntimeOrigin::signed(3), lock));
			System::assert_last_event(RuntimeEvent::CollatorStaking(Event::LockExtended {
				account: 3,
				amount: lock,
			}));
			for _ in 0..release_queue_max_len {
				assert_ok!(CollatorStaking::unlock(RuntimeOrigin::signed(3), Some(2)));
			}
			assert_eq!(ReleaseQueues::<Test>::get(3).len() as u32, release_queue_max_len);
			assert_ok!(CollatorStaking::leave_intent(RuntimeOrigin::signed(3)));
		});
	}
}

mod stake {
	use super::*;

	#[test]
	fn cannot_stake_with_empty_target() {
		new_test_ext().execute_with(|| {
			initialize_to_block(1);

			register_candidates(3..=3);
			lock_for_staking(4..=4);

			// Attempt to stake with an empty target vector
			assert_noop!(
				CollatorStaking::stake(RuntimeOrigin::signed(4), vec![].try_into().unwrap()),
				Error::<Test>::TooFewCandidates
			);
		});
	}

	#[test]
	fn cannot_stake_if_not_candidate() {
		new_test_ext().execute_with(|| {
			initialize_to_block(1);

			lock_for_staking(4..=4);
			// invulnerable
			assert_noop!(
				CollatorStaking::stake(
					RuntimeOrigin::signed(4),
					vec![StakeTarget { candidate: 1, stake: 1 }].try_into().unwrap()
				),
				Error::<Test>::NotCandidate
			);
			// not registered as candidate
			assert_noop!(
				CollatorStaking::stake(
					RuntimeOrigin::signed(4),
					vec![StakeTarget { candidate: 5, stake: 15 }].try_into().unwrap()
				),
				Error::<Test>::NotCandidate
			);
		});
	}

	#[test]
	fn cannot_stake_if_recently_unstaked() {
		new_test_ext().execute_with(|| {
			initialize_to_block(1);

			register_candidates(3..=3);
			assert_ok!(CollatorStaking::lock(RuntimeOrigin::signed(3), 20));
			System::assert_last_event(RuntimeEvent::CollatorStaking(Event::LockExtended {
				account: 3,
				amount: 20,
			}));
			assert_ok!(CollatorStaking::stake(
				RuntimeOrigin::signed(3),
				vec![StakeTarget { candidate: 3, stake: 20 }].try_into().unwrap()
			));
			assert_eq!(
				UserStake::<Test>::get(3),
				UserStakeInfo {
					stake: 20,
					candidates: bbtreeset![3],
					maybe_last_unstake: None,
					maybe_last_reward_session: Some(0),
				}
			);
			assert_ok!(CollatorStaking::unstake_from(RuntimeOrigin::signed(3), 3));
			assert_eq!(
				UserStake::<Test>::get(3),
				UserStakeInfo {
					stake: 0,
					candidates: bbtreeset![],
					maybe_last_unstake: Some((20, 11)),
					maybe_last_reward_session: None,
				}
			);
			assert_noop!(
				CollatorStaking::stake(
					RuntimeOrigin::signed(3),
					vec![StakeTarget { candidate: 3, stake: 20 }].try_into().unwrap()
				),
				Error::<Test>::InsufficientLockedBalance
			);

			// In the future we can stake again
			initialize_to_block(12);
			assert_ok!(CollatorStaking::stake(
				RuntimeOrigin::signed(3),
				vec![StakeTarget { candidate: 3, stake: 20 }].try_into().unwrap()
			));
			assert_eq!(
				UserStake::<Test>::get(3),
				UserStakeInfo {
					stake: 20,
					candidates: bbtreeset![3],
					maybe_last_unstake: None,
					maybe_last_reward_session: Some(1),
				}
			);
		});
	}

	#[test]
	fn cannot_stake_if_under_minstake() {
		new_test_ext().execute_with(|| {
			initialize_to_block(1);

			register_candidates(3..=3);
			lock_for_staking(4..=4);
			assert_noop!(
				CollatorStaking::stake(
					RuntimeOrigin::signed(4),
					vec![StakeTarget { candidate: 3, stake: 1 }].try_into().unwrap()
				),
				Error::<Test>::InsufficientStake
			);
			assert_ok!(CollatorStaking::stake(
				RuntimeOrigin::signed(4),
				vec![StakeTarget { candidate: 3, stake: 2 }].try_into().unwrap()
			));
			assert_eq!(Balances::balance_frozen(&FreezeReason::Staking.into(), &4), 100);
			assert_eq!(
				CandidateStake::<Test>::get(3, 4),
				CandidateStakeInfo { stake: 2, session: 0 }
			);

			// After adding MinStake it should work
			assert_ok!(CollatorStaking::stake(
				RuntimeOrigin::signed(4),
				vec![StakeTarget { candidate: 3, stake: 1 }].try_into().unwrap()
			));
			assert_eq!(Balances::balance_frozen(&FreezeReason::Staking.into(), &4), 100);
			assert_eq!(
				CandidateStake::<Test>::get(3, 4),
				CandidateStakeInfo { stake: 3, session: 0 }
			);
		});
	}

	#[test]
	fn stake() {
		new_test_ext().execute_with(|| {
			initialize_to_block(1);

			register_candidates(3..=3);
			assert_eq!(Balances::balance_frozen(&FreezeReason::CandidacyBond.into(), &3), 10);
			assert_eq!(Balances::balance_frozen(&FreezeReason::Staking.into(), &3), 0);
			lock_for_staking(3..=3);
			assert_eq!(Balances::balance_frozen(&FreezeReason::CandidacyBond.into(), &3), 10);
			assert_eq!(Balances::balance_frozen(&FreezeReason::Staking.into(), &3), 90);

			assert_eq!(
				CandidateStake::<Test>::get(3, 3),
				CandidateStakeInfo { stake: 0, session: 0 }
			);
			assert_eq!(
				UserStake::<Test>::get(3),
				UserStakeInfo {
					stake: 0,
					candidates: bbtreeset![],
					maybe_last_unstake: None,
					maybe_last_reward_session: None,
				}
			);
			assert_eq!(Candidates::<Test>::iter_values().next().unwrap().stake, 0);

			assert_eq!(Balances::balance_frozen(&FreezeReason::Staking.into(), &4), 0);
			assert_eq!(
				UserStake::<Test>::get(4),
				UserStakeInfo {
					stake: 0,
					candidates: bbtreeset![],
					maybe_last_unstake: None,
					maybe_last_reward_session: None,
				}
			);
			lock_for_staking(4..=4);
			assert_ok!(CollatorStaking::stake(
				RuntimeOrigin::signed(4),
				vec![StakeTarget { candidate: 3, stake: 20 }].try_into().unwrap()
			));
			assert_eq!(Balances::balance_frozen(&FreezeReason::Staking.into(), &4), 100);
			System::assert_has_event(RuntimeEvent::CollatorStaking(Event::StakeAdded {
				account: 4,
				candidate: 3,
				amount: 20,
			}));
			assert_eq!(
				CandidateStake::<Test>::get(3, 4),
				CandidateStakeInfo { stake: 20, session: 0 }
			);
			assert_eq!(
				CandidateStake::<Test>::get(3, 3),
				CandidateStakeInfo { stake: 0, session: 0 }
			);
			assert_eq!(Candidates::<Test>::iter_values().next().unwrap().stake, 20);
			assert_eq!(
				UserStake::<Test>::get(4),
				UserStakeInfo {
					stake: 20,
					candidates: bbtreeset![3],
					maybe_last_unstake: None,
					maybe_last_reward_session: Some(0)
				}
			);
		});
	}

	#[test]
	fn stake_many_at_once() {
		new_test_ext().execute_with(|| {
			initialize_to_block(1);

			register_candidates(3..=4);
			lock_for_staking(3..=3);

			assert_eq!(Balances::balance_frozen(&FreezeReason::Staking.into(), &3), 90);
			assert_eq!(
				CandidateStake::<Test>::get(3, 3),
				CandidateStakeInfo { stake: 0, session: 0 }
			);
			assert_eq!(
				CandidateStake::<Test>::get(3, 4),
				CandidateStakeInfo { stake: 0, session: 0 }
			);
			assert_eq!(
				UserStake::<Test>::get(3),
				UserStakeInfo {
					stake: 0,
					candidates: bbtreeset![],
					maybe_last_unstake: None,
					maybe_last_reward_session: None,
				}
			);

			assert_ok!(CollatorStaking::stake(
				RuntimeOrigin::signed(3),
				vec![
					StakeTarget { candidate: 3, stake: 20 },
					StakeTarget { candidate: 4, stake: 20 },
				]
				.try_into()
				.unwrap()
			));
			System::assert_has_event(RuntimeEvent::CollatorStaking(Event::StakeAdded {
				account: 3,
				candidate: 4,
				amount: 20,
			}));
			System::assert_has_event(RuntimeEvent::CollatorStaking(Event::StakeAdded {
				account: 3,
				candidate: 3,
				amount: 20,
			}));
			assert_eq!(
				CandidateStake::<Test>::get(4, 3),
				CandidateStakeInfo { stake: 20, session: 0 }
			);
			assert_eq!(
				CandidateStake::<Test>::get(3, 3),
				CandidateStakeInfo { stake: 20, session: 0 }
			);
			assert_eq!(
				UserStake::<Test>::get(3),
				UserStakeInfo {
					stake: 40,
					candidates: bbtreeset![3, 4],
					maybe_last_unstake: None,
					maybe_last_reward_session: Some(0)
				}
			);
		});
	}

	#[test]
	fn stake_many_over_limits_should_fail() {
		new_test_ext().execute_with(|| {
			initialize_to_block(1);

			register_candidates(3..=4);
			lock_for_staking(3..=3);

			assert_eq!(Balances::balance_frozen(&FreezeReason::Staking.into(), &3), 90);
			assert_eq!(
				CandidateStake::<Test>::get(3, 3),
				CandidateStakeInfo { stake: 0, session: 0 }
			);
			assert_eq!(
				CandidateStake::<Test>::get(3, 4),
				CandidateStakeInfo { stake: 0, session: 0 }
			);
			assert_eq!(
				UserStake::<Test>::get(3),
				UserStakeInfo {
					stake: 0,
					candidates: bbtreeset![],
					maybe_last_unstake: None,
					maybe_last_reward_session: None,
				}
			);

			assert_noop!(
				CollatorStaking::stake(
					RuntimeOrigin::signed(3),
					vec![
						StakeTarget { candidate: 3, stake: 20 },
						StakeTarget { candidate: 4, stake: 90 },
					]
					.try_into()
					.unwrap()
				),
				Error::<Test>::InsufficientLockedBalance
			);
		});
	}

	#[test]
	fn stake_and_reassign_position() {
		new_test_ext().execute_with(|| {
			initialize_to_block(1);

			register_candidates(3..=4);

			assert_eq!(CollatorStaking::get_staked_balance(&5), 0);
			assert_eq!(CollatorStaking::get_free_balance(&5), 100);
			assert_ok!(CollatorStaking::lock(RuntimeOrigin::signed(5), 60));
			System::assert_last_event(RuntimeEvent::CollatorStaking(Event::LockExtended {
				account: 5,
				amount: 60,
			}));
			assert_eq!(CollatorStaking::get_staked_balance(&5), 60);
			assert_eq!(CollatorStaking::get_free_balance(&5), 40);

			assert_eq!(
				candidate_list(),
				vec![
					(3, CandidateInfo { stake: 0, stakers: 0 }),
					(4, CandidateInfo { stake: 0, stakers: 0 }),
				]
			);
			assert_ok!(CollatorStaking::stake(
				RuntimeOrigin::signed(5),
				vec![StakeTarget { candidate: 3, stake: 12 }].try_into().unwrap()
			));
			assert_eq!(
				candidate_list(),
				vec![
					(4, CandidateInfo { stake: 0, stakers: 0 }),
					(3, CandidateInfo { stake: 12, stakers: 1 }),
				]
			);

			assert_ok!(CollatorStaking::stake(
				RuntimeOrigin::signed(5),
				vec![StakeTarget { candidate: 4, stake: 15 }].try_into().unwrap()
			));
			assert_eq!(
				candidate_list(),
				vec![
					(3, CandidateInfo { stake: 12, stakers: 1 }),
					(4, CandidateInfo { stake: 15, stakers: 1 }),
				]
			);

			register_candidates(5..=5);
			assert_eq!(
				candidate_list(),
				vec![
					(5, CandidateInfo { stake: 0, stakers: 0 }),
					(3, CandidateInfo { stake: 12, stakers: 1 }),
					(4, CandidateInfo { stake: 15, stakers: 1 }),
				]
			);
			assert_ok!(CollatorStaking::stake(
				RuntimeOrigin::signed(5),
				vec![StakeTarget { candidate: 5, stake: 13 }].try_into().unwrap()
			));
			assert_eq!(
				candidate_list(),
				vec![
					(3, CandidateInfo { stake: 12, stakers: 1 }),
					(5, CandidateInfo { stake: 13, stakers: 1 }),
					(4, CandidateInfo { stake: 15, stakers: 1 }),
				]
			);
			assert_ok!(CollatorStaking::stake(
				RuntimeOrigin::signed(5),
				vec![StakeTarget { candidate: 5, stake: 7 }].try_into().unwrap()
			));
			assert_eq!(
				candidate_list(),
				vec![
					(3, CandidateInfo { stake: 12, stakers: 1 }),
					(4, CandidateInfo { stake: 15, stakers: 1 }),
					(5, CandidateInfo { stake: 20, stakers: 1 }),
				]
			);
		});
	}

	#[test]
	fn cannot_stake_too_many_staked_candidates() {
		new_test_ext().execute_with(|| {
			initialize_to_block(1);

			assert_eq!(<Test as Config>::MaxStakedCandidates::get(), 16);

			register_candidates(3..=19);
			lock_for_staking(1..=1);
			for i in 3..=18 {
				assert_ok!(CollatorStaking::stake(
					RuntimeOrigin::signed(1),
					vec![StakeTarget { candidate: i, stake: 2 }].try_into().unwrap()
				));
			}
			assert_eq!(
				UserStake::<Test>::get(1),
				UserStakeInfo {
					stake: 32,
					candidates: bbtreeset![3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18],
					maybe_last_unstake: None,
					maybe_last_reward_session: Some(0)
				}
			);
			assert_noop!(
				CollatorStaking::stake(
					RuntimeOrigin::signed(1),
					vec![StakeTarget { candidate: 19, stake: 2 }].try_into().unwrap()
				),
				Error::<Test>::TooManyStakedCandidates
			);
		});
	}

	#[test]
	fn cannot_stake_too_many_stakers() {
		new_test_ext().execute_with(|| {
			initialize_to_block(1);

			assert_eq!(<Test as Config>::MaxStakers::get(), 25);

			register_candidates(3..=3);
			for i in 4..=28 {
				fund_account(i);
				lock_for_staking(i..=i);
				assert_ok!(CollatorStaking::stake(
					RuntimeOrigin::signed(i),
					vec![StakeTarget { candidate: 3, stake: 2 }].try_into().unwrap()
				));
			}
			assert_eq!(candidate_list()[0].1.stakers, 25);
			fund_account(29);
			lock_for_staking(29..=29);
			assert_noop!(
				CollatorStaking::stake(
					RuntimeOrigin::signed(29),
					vec![StakeTarget { candidate: 3, stake: 2 }].try_into().unwrap()
				),
				Error::<Test>::TooManyStakers
			);
		});
	}

	#[test]
	fn cannot_stake_invulnerable() {
		new_test_ext().execute_with(|| {
			initialize_to_block(1);

			lock_for_staking(3..=3);
			assert_noop!(
				CollatorStaking::stake(
					RuntimeOrigin::signed(3),
					vec![StakeTarget { candidate: 1, stake: 2 }].try_into().unwrap()
				),
				Error::<Test>::NotCandidate
			);
		});
	}

	#[test]
	fn must_claim_before_stake() {
		new_test_ext().execute_with(|| {
			initialize_to_block(1);

			register_candidates(3..=4);
			lock_for_staking(5..=5);
			assert_eq!(
				UserStake::<Test>::get(5),
				UserStakeInfo {
					stake: 0,
					candidates: bbtreeset![],
					maybe_last_unstake: None,
					maybe_last_reward_session: None,
				}
			);
			assert_ok!(CollatorStaking::stake(
				RuntimeOrigin::signed(5),
				vec![StakeTarget { candidate: 3, stake: 20 }].try_into().unwrap()
			));

			// Time travel to the next Session
			initialize_to_block(10);
			assert_eq!(CurrentSession::<Test>::get(), 1);
			assert_noop!(
				CollatorStaking::stake(
					RuntimeOrigin::signed(5),
					vec![StakeTarget { candidate: 4, stake: 10 }].try_into().unwrap()
				),
				Error::<Test>::PreviousRewardsNotClaimed
			);

			// Claim and retry operation
			assert_ok!(CollatorStaking::claim_rewards(RuntimeOrigin::signed(5)));
			assert_ok!(CollatorStaking::stake(
				RuntimeOrigin::signed(5),
				vec![StakeTarget { candidate: 4, stake: 10 }].try_into().unwrap()
			));
		});
	}

	#[test]
	fn lock_stake_unstake_unlock_and_stake_again() {
		new_test_ext().execute_with(|| {
			initialize_to_block(1);

			// Lock balance for staking
			assert_ok!(CollatorStaking::lock(RuntimeOrigin::signed(5), 30));
			assert_eq!(CollatorStaking::get_staked_balance(&5), 30);
			System::assert_last_event(RuntimeEvent::CollatorStaking(Event::LockExtended {
				account: 5,
				amount: 30,
			}));

			// Register candidates
			register_candidates(3..=3);

			// Stake funds
			assert_ok!(CollatorStaking::stake(
				RuntimeOrigin::signed(5),
				vec![StakeTarget { candidate: 3, stake: 20 }].try_into().unwrap()
			));
			assert_eq!(candidate_list(), vec![(3, CandidateInfo { stake: 20, stakers: 1 }),]);
			assert_eq!(CollatorStaking::get_staked_balance(&5), 30);
			assert_eq!(
				UserStake::<Test>::get(5),
				UserStakeInfo {
					stake: 20,
					candidates: bbtreeset![3],
					maybe_last_unstake: None,
					maybe_last_reward_session: Some(0),
				}
			);

			// Unstake
			assert_ok!(CollatorStaking::unstake_from(RuntimeOrigin::signed(5), 3));
			assert_eq!(candidate_list(), vec![(3, CandidateInfo { stake: 0, stakers: 0 }),]);
			assert_eq!(CollatorStaking::get_staked_balance(&5), 30);
			assert_eq!(
				UserStake::<Test>::get(5),
				UserStakeInfo {
					stake: 0,
					candidates: bbtreeset![],
					maybe_last_unstake: Some((20, 11)),
					maybe_last_reward_session: None,
				}
			);

			// Now we have a penalty of 20, and we staked 30. This implies we can now stake up to 10
			assert_noop!(
				CollatorStaking::stake(
					RuntimeOrigin::signed(5),
					vec![StakeTarget { candidate: 3, stake: 11 }].try_into().unwrap()
				),
				Error::<Test>::InsufficientLockedBalance
			);
			assert_ok!(CollatorStaking::stake(
				RuntimeOrigin::signed(5),
				vec![StakeTarget { candidate: 3, stake: 10 }].try_into().unwrap()
			));
			assert_eq!(
				UserStake::<Test>::get(5),
				UserStakeInfo {
					stake: 10,
					candidates: bbtreeset![3],
					maybe_last_unstake: Some((20, 11)),
					maybe_last_reward_session: Some(0),
				}
			);

			// Unstake and unlock all balance
			assert_ok!(CollatorStaking::unstake_from(RuntimeOrigin::signed(5), 3));
			assert_eq!(
				UserStake::<Test>::get(5),
				UserStakeInfo {
					stake: 0,
					candidates: bbtreeset![],
					maybe_last_unstake: Some((30, 11)),
					maybe_last_reward_session: None,
				}
			);
			assert_ok!(CollatorStaking::unlock(RuntimeOrigin::signed(5), None));
			assert_eq!(CollatorStaking::get_staked_balance(&5), 0);
			assert_eq!(
				UserStake::<Test>::get(5),
				UserStakeInfo {
					stake: 0,
					candidates: bbtreeset![],
					maybe_last_unstake: None,
					maybe_last_reward_session: None,
				}
			);

			// Lock and stake again after unlocking
			assert_ok!(CollatorStaking::lock(RuntimeOrigin::signed(5), 10));
			assert_eq!(CollatorStaking::get_staked_balance(&5), 10);
			System::assert_last_event(RuntimeEvent::CollatorStaking(Event::LockExtended {
				account: 5,
				amount: 10,
			}));

			assert_ok!(CollatorStaking::stake(
				RuntimeOrigin::signed(5),
				vec![StakeTarget { candidate: 3, stake: 10 }].try_into().unwrap()
			));
			assert_eq!(candidate_list(), vec![(3, CandidateInfo { stake: 10, stakers: 1 }),]);
			assert_eq!(CollatorStaking::get_staked_balance(&5), 10);
			assert_eq!(
				UserStake::<Test>::get(5),
				UserStakeInfo {
					stake: 10,
					candidates: bbtreeset![3],
					maybe_last_unstake: None,
					maybe_last_reward_session: Some(0),
				}
			);
		});
	}

	#[test]
	fn stake_in_one_then_unstake_in_another_and_fail_to_restake_in_original() {
		new_test_ext().execute_with(|| {
			initialize_to_block(1);

			// Register candidates 3 and 4
			register_candidates(3..=4);
			assert_ok!(CollatorStaking::lock(RuntimeOrigin::signed(5), 30));
			System::assert_last_event(RuntimeEvent::CollatorStaking(Event::LockExtended {
				account: 5,
				amount: 30,
			}));

			// Ensure initial user stake state is empty
			assert_eq!(
				UserStake::<Test>::get(5),
				UserStakeInfo {
					stake: 0,
					candidates: bbtreeset![],
					maybe_last_unstake: None,
					maybe_last_reward_session: None,
				}
			);

			// Stake 20 on candidate 3
			assert_ok!(CollatorStaking::stake(
				RuntimeOrigin::signed(5),
				vec![StakeTarget { candidate: 3, stake: 20 }].try_into().unwrap()
			));

			// Stake 10 on candidate 4
			assert_ok!(CollatorStaking::stake(
				RuntimeOrigin::signed(5),
				vec![StakeTarget { candidate: 4, stake: 10 }].try_into().unwrap()
			));

			// Validate the candidate list
			assert_eq!(
				candidate_list(),
				vec![
					(4, CandidateInfo { stake: 10, stakers: 1 }),
					(3, CandidateInfo { stake: 20, stakers: 1 }),
				]
			);

			// Unstake 10 from candidate 4
			assert_ok!(CollatorStaking::unstake_from(RuntimeOrigin::signed(5), 4));

			// Validate the updated user stake state
			assert_eq!(
				UserStake::<Test>::get(5),
				UserStakeInfo {
					stake: 20,
					candidates: bbtreeset![3],
					maybe_last_unstake: Some((10, 11)),
					maybe_last_reward_session: Some(0),
				}
			);

			// Attempt to stake the unstaked 10 back into candidate 3 (should fail)
			assert_noop!(
				CollatorStaking::stake(
					RuntimeOrigin::signed(5),
					vec![StakeTarget { candidate: 3, stake: 10 }].try_into().unwrap()
				),
				Error::<Test>::InsufficientLockedBalance
			);
		});
	}
}

mod edge_case_tests {
	use super::*;

	#[test]
	fn stake_zero_amount_fails() {
		new_test_ext().execute_with(|| {
			initialize_to_block(1);
			register_candidates(3..=3);

			// Attempt to stake with zero amount
			assert_noop!(
				CollatorStaking::stake(
					RuntimeOrigin::signed(5),
					vec![StakeTarget { candidate: 3, stake: 0 }].try_into().unwrap()
				),
				Error::<Test>::InsufficientStake
			);
		});
	}

	#[test]
	fn unstake_from_nonexistent_candidate() {
		new_test_ext().execute_with(|| {
			initialize_to_block(1);

			register_candidates(3..=4);
			lock_for_staking(5..=5);
			// User stakes 30 on candidate 3
			assert_ok!(CollatorStaking::stake(
				RuntimeOrigin::signed(5),
				vec![StakeTarget { candidate: 3, stake: 30 }].try_into().unwrap()
			));
			// Attempt to unstake from a non-existent candidate (e.g. candidate 42)
			assert_noop!(
				CollatorStaking::unstake_from(RuntimeOrigin::signed(5), 42),
				Error::<Test>::NoStakeOnCandidate
			);
		});
	}

	#[test]
	fn stake_more_than_locked_balance_fails() {
		new_test_ext().execute_with(|| {
			initialize_to_block(1);

			register_candidates(3..=4);
			lock_for_staking(5..=5);
			// Attempt to stake more than user's locked balance
			assert_noop!(
				CollatorStaking::stake(
					RuntimeOrigin::signed(5),
					vec![StakeTarget { candidate: 3, stake: 110 }].try_into().unwrap()
				),
				Error::<Test>::InsufficientLockedBalance
			);
		});
	}

	#[test]
	fn restaking_after_full_unstake() {
		new_test_ext().execute_with(|| {
			initialize_to_block(1);

			register_candidates(3..=3);
			lock_for_staking(5..=5);

			// User stakes 30 on candidate 3
			assert_ok!(CollatorStaking::stake(
				RuntimeOrigin::signed(5),
				vec![StakeTarget { candidate: 3, stake: 30 }].try_into().unwrap()
			));

			// Fully unstake from candidate 3
			assert_ok!(CollatorStaking::unstake_from(RuntimeOrigin::signed(5), 3));
			assert_eq!(
				UserStake::<Test>::get(5),
				UserStakeInfo {
					stake: 0,
					candidates: bbtreeset![],
					maybe_last_unstake: Some((30, 11)),
					maybe_last_reward_session: None,
				}
			);

			// Restake on candidate 3 after full unstake
			assert_ok!(CollatorStaking::stake(
				RuntimeOrigin::signed(5),
				vec![StakeTarget { candidate: 3, stake: 20 }].try_into().unwrap()
			));

			// Validate new stakes and user state
			assert_eq!(candidate_list(), vec![(3, CandidateInfo { stake: 20, stakers: 1 })]);
			assert_eq!(
				UserStake::<Test>::get(5),
				UserStakeInfo {
					stake: 20,
					candidates: bbtreeset![3],
					maybe_last_unstake: Some((30, 11)),
					maybe_last_reward_session: Some(0),
				}
			);
		});
	}
}

mod claim_rewards {
	use super::*;

	#[test]
	fn cannot_claim_if_on_same_session() {
		new_test_ext().execute_with(|| {
			initialize_to_block(1);

			register_candidates(3..=4);
			lock_for_staking(5..=5);
			assert_eq!(
				UserStake::<Test>::get(5),
				UserStakeInfo {
					stake: 0,
					candidates: bbtreeset![],
					maybe_last_unstake: None,
					maybe_last_reward_session: None,
				}
			);
			let pre_stake_session = CurrentSession::<Test>::get();

			assert_ok!(CollatorStaking::stake(
				RuntimeOrigin::signed(5),
				vec![StakeTarget { candidate: 3, stake: 20 }].try_into().unwrap()
			));

			// Attempt claim in same session
			assert_eq!(CurrentSession::<Test>::get(), pre_stake_session);
			assert_noop!(
				CollatorStaking::claim_rewards(RuntimeOrigin::signed(5)),
				Error::<Test>::NoPendingClaim
			);

			// Time travel to next session
			initialize_to_block(10);
			assert_eq!(CurrentSession::<Test>::get(), pre_stake_session + 1);
			assert_ok!(CollatorStaking::claim_rewards(RuntimeOrigin::signed(5)));
		});
	}
}

mod unstake_from {
	use super::*;

	#[test]
	fn unstake_from_candidate() {
		new_test_ext().execute_with(|| {
			initialize_to_block(1);

			register_candidates(3..=4);
			lock_for_staking(5..=5);
			assert_eq!(
				UserStake::<Test>::get(5),
				UserStakeInfo {
					stake: 0,
					candidates: bbtreeset![],
					maybe_last_unstake: None,
					maybe_last_reward_session: None,
				}
			);
			assert_ok!(CollatorStaking::stake(
				RuntimeOrigin::signed(5),
				vec![StakeTarget { candidate: 3, stake: 20 }].try_into().unwrap()
			));
			assert_ok!(CollatorStaking::stake(
				RuntimeOrigin::signed(5),
				vec![StakeTarget { candidate: 4, stake: 10 }].try_into().unwrap()
			));
			assert_eq!(
				candidate_list(),
				vec![
					(4, CandidateInfo { stake: 10, stakers: 1 }),
					(3, CandidateInfo { stake: 20, stakers: 1 }),
				]
			);

			// unstake from actual candidate
			assert_eq!(Balances::balance_frozen(&FreezeReason::Staking.into(), &5), 100);
			assert_eq!(
				UserStake::<Test>::get(5),
				UserStakeInfo {
					stake: 30,
					candidates: bbtreeset![3, 4],
					maybe_last_unstake: None,
					maybe_last_reward_session: Some(0)
				}
			);
			assert_ok!(CollatorStaking::unstake_from(RuntimeOrigin::signed(5), 3));
			System::assert_last_event(RuntimeEvent::CollatorStaking(Event::StakeRemoved {
				account: 5,
				candidate: 3,
				amount: 20,
			}));
			// candidate list gets reordered
			assert_eq!(
				candidate_list(),
				vec![
					(3, CandidateInfo { stake: 0, stakers: 0 }),
					(4, CandidateInfo { stake: 10, stakers: 1 }),
				]
			);
			assert_eq!(
				UserStake::<Test>::get(5),
				UserStakeInfo {
					stake: 10,
					candidates: bbtreeset![4],
					maybe_last_unstake: Some((20, 11)),
					maybe_last_reward_session: Some(0)
				}
			);
			assert_eq!(
				CandidateStake::<Test>::get(3, 5),
				CandidateStakeInfo { stake: 0, session: 0 }
			);
			assert_eq!(
				CandidateStake::<Test>::get(4, 5),
				CandidateStakeInfo { stake: 10, session: 0 }
			);
			assert_eq!(Balances::balance_frozen(&FreezeReason::Staking.into(), &5), 100);
			assert_eq!(ReleaseQueues::<Test>::get(5), vec![]);
		});
	}

	#[test]
	fn unstake_self() {
		new_test_ext().execute_with(|| {
			initialize_to_block(1);

			assert_eq!(
				UserStake::<Test>::get(3),
				UserStakeInfo {
					stake: 0,
					candidates: bbtreeset![],
					maybe_last_unstake: None,
					maybe_last_reward_session: None,
				}
			);
			assert_eq!(Balances::balance(&3), 100);
			assert_eq!(MinCandidacyBond::<Test>::get(), 10);
			register_candidates(3..=4);
			assert_eq!(Balances::balance_frozen(&FreezeReason::CandidacyBond.into(), &3), 10);

			lock_for_staking(3..=3);
			assert_eq!(
				UserStake::<Test>::get(3),
				UserStakeInfo {
					stake: 0,
					candidates: bbtreeset![],
					maybe_last_unstake: None,
					maybe_last_reward_session: None,
				}
			);

			assert_ok!(CollatorStaking::stake(
				RuntimeOrigin::signed(3),
				vec![StakeTarget { candidate: 3, stake: 20 }].try_into().unwrap()
			));
			assert_eq!(Balances::balance_frozen(&FreezeReason::Staking.into(), &3), 90);
			assert_eq!(
				UserStake::<Test>::get(3),
				UserStakeInfo {
					stake: 20,
					candidates: bbtreeset![3],
					maybe_last_unstake: None,
					maybe_last_reward_session: Some(0),
				}
			);

			assert_ok!(CollatorStaking::stake(
				RuntimeOrigin::signed(3),
				vec![StakeTarget { candidate: 4, stake: 10 }].try_into().unwrap()
			));
			assert_eq!(Balances::balance_frozen(&FreezeReason::Staking.into(), &3), 90);
			assert_eq!(
				UserStake::<Test>::get(3),
				UserStakeInfo {
					stake: 30,
					candidates: bbtreeset![3, 4],
					maybe_last_unstake: None,
					maybe_last_reward_session: Some(0),
				}
			);

			assert_eq!(
				candidate_list(),
				vec![
					(4, CandidateInfo { stake: 10, stakers: 1 }),
					(3, CandidateInfo { stake: 20, stakers: 1 }),
				]
			);

			// unstake from actual candidate
			assert_ok!(CollatorStaking::unstake_from(RuntimeOrigin::signed(3), 3));
			System::assert_last_event(RuntimeEvent::CollatorStaking(Event::StakeRemoved {
				account: 3,
				candidate: 3,
				amount: 20,
			}));
			assert_eq!(
				candidate_list(),
				vec![
					(3, CandidateInfo { stake: 0, stakers: 0 }),
					(4, CandidateInfo { stake: 10, stakers: 1 }),
				]
			);
			assert_eq!(
				UserStake::<Test>::get(3),
				UserStakeInfo {
					stake: 10,
					candidates: bbtreeset![4],
					maybe_last_unstake: Some((20, 11)),
					maybe_last_reward_session: Some(0),
				}
			);
			assert_eq!(
				CandidateStake::<Test>::get(3, 3),
				CandidateStakeInfo { stake: 0, session: 0 }
			);
			assert_eq!(
				CandidateStake::<Test>::get(4, 3),
				CandidateStakeInfo { stake: 10, session: 0 }
			);
			assert_eq!(Balances::balance_frozen(&FreezeReason::Staking.into(), &3), 90);
			assert_eq!(Balances::balance_frozen(&FreezeReason::CandidacyBond.into(), &3), 10);
			assert_eq!(ReleaseQueues::<Test>::get(3), vec![]);

			// check after unstaking with a shorter delay the list remains sorted by block
			assert_ok!(CollatorStaking::unstake_from(RuntimeOrigin::signed(3), 4));
			assert_eq!(ReleaseQueues::<Test>::get(3), vec![]);
		});
	}

	#[test]
	fn unstake_from_ex_candidate() {
		new_test_ext().execute_with(|| {
			initialize_to_block(1);

			register_candidates(3..=4);
			assert_eq!(
				UserStake::<Test>::get(5),
				UserStakeInfo {
					stake: 0,
					candidates: bbtreeset![],
					maybe_last_unstake: None,
					maybe_last_reward_session: None,
				}
			);
			lock_for_staking(5..=5);
			assert_ok!(CollatorStaking::stake(
				RuntimeOrigin::signed(5),
				vec![StakeTarget { candidate: 3, stake: 20 }].try_into().unwrap()
			));
			assert_ok!(CollatorStaking::stake(
				RuntimeOrigin::signed(5),
				vec![StakeTarget { candidate: 4, stake: 10 }].try_into().unwrap()
			));
			assert_eq!(
				candidate_list(),
				vec![
					(4, CandidateInfo { stake: 10, stakers: 1 }),
					(3, CandidateInfo { stake: 20, stakers: 1 }),
				]
			);
			assert_eq!(
				CandidateStake::<Test>::get(3, 5),
				CandidateStakeInfo { stake: 20, session: 0 }
			);
			assert_eq!(
				CandidateStake::<Test>::get(4, 5),
				CandidateStakeInfo { stake: 10, session: 0 }
			);

			// unstake from ex-candidate.
			assert_eq!(
				UserStake::<Test>::get(5),
				UserStakeInfo {
					stake: 30,
					candidates: bbtreeset![3, 4],
					maybe_last_unstake: None,
					maybe_last_reward_session: Some(0),
				}
			);
			assert_ok!(CollatorStaking::leave_intent(RuntimeOrigin::signed(3)));
			assert_eq!(candidate_list(), vec![(4, CandidateInfo { stake: 10, stakers: 1 })]);

			// the stake should be the same.
			assert_eq!(
				UserStake::<Test>::get(5),
				UserStakeInfo {
					stake: 30,
					candidates: bbtreeset![3, 4],
					maybe_last_unstake: None,
					maybe_last_reward_session: Some(0),
				}
			);
			assert_eq!(Balances::balance_frozen(&FreezeReason::Staking.into(), &5), 100);
			assert_ok!(CollatorStaking::unstake_from(RuntimeOrigin::signed(5), 3));
		});
	}

	#[test]
	fn must_claim_before_unstake_from() {
		new_test_ext().execute_with(|| {
			initialize_to_block(1);

			register_candidates(3..=4);
			lock_for_staking(5..=5);
			assert_eq!(
				UserStake::<Test>::get(5),
				UserStakeInfo {
					stake: 0,
					candidates: bbtreeset![],
					maybe_last_unstake: None,
					maybe_last_reward_session: None,
				}
			);
			assert_ok!(CollatorStaking::stake(
				RuntimeOrigin::signed(5),
				vec![StakeTarget { candidate: 3, stake: 20 }].try_into().unwrap()
			));

			// Time travel to the next Session
			initialize_to_block(10);
			assert_eq!(CurrentSession::<Test>::get(), 1);
			assert_noop!(
				CollatorStaking::unstake_from(RuntimeOrigin::signed(5), 3),
				Error::<Test>::PreviousRewardsNotClaimed
			);

			// Claim and retry operation
			assert_ok!(CollatorStaking::claim_rewards(RuntimeOrigin::signed(5)));
			assert_ok!(CollatorStaking::unstake_from(RuntimeOrigin::signed(5), 3));
		});
	}

	#[test]
	fn unstakes_accumulates_amount() {
		new_test_ext().execute_with(|| {
			initialize_to_block(1);

			register_candidates(3..=3);
			lock_for_staking(5..=5);

			// Not staked yet
			assert_eq!(
				UserStake::<Test>::get(5),
				UserStakeInfo {
					stake: 0,
					candidates: bbtreeset![],
					maybe_last_unstake: None,
					maybe_last_reward_session: None,
				}
			);

			// First stake
			assert_ok!(CollatorStaking::stake(
				RuntimeOrigin::signed(5),
				vec![StakeTarget { candidate: 3, stake: 20 }].try_into().unwrap()
			));
			assert_eq!(
				UserStake::<Test>::get(5),
				UserStakeInfo {
					stake: 20,
					candidates: bbtreeset![3],
					maybe_last_unstake: None,
					maybe_last_reward_session: Some(0),
				}
			);

			// First unstake
			assert_ok!(CollatorStaking::unstake_from(RuntimeOrigin::signed(5), 3));
			assert_eq!(
				UserStake::<Test>::get(5),
				UserStakeInfo {
					stake: 0,
					candidates: bbtreeset![],
					maybe_last_unstake: Some((20, 11)),
					maybe_last_reward_session: None,
				}
			);

			// Moving one block
			initialize_to_block(2);

			// Second stake
			assert_ok!(CollatorStaking::stake(
				RuntimeOrigin::signed(5),
				vec![StakeTarget { candidate: 3, stake: 20 }].try_into().unwrap()
			));
			assert_eq!(
				UserStake::<Test>::get(5),
				UserStakeInfo {
					stake: 20,
					candidates: bbtreeset![3],
					maybe_last_unstake: Some((20, 11)),
					maybe_last_reward_session: Some(0),
				}
			);

			// Second unstake
			assert_ok!(CollatorStaking::unstake_from(RuntimeOrigin::signed(5), 3));
			assert_eq!(
				UserStake::<Test>::get(5),
				UserStakeInfo {
					stake: 0,
					candidates: bbtreeset![],
					maybe_last_unstake: Some((40, 12)),
					maybe_last_reward_session: None,
				}
			);
		});
	}
}

mod unstake_all {
	use super::*;

	#[test]
	fn unstake_all() {
		new_test_ext().execute_with(|| {
			initialize_to_block(1);

			register_candidates(3..=4);
			lock_for_staking(5..=5);
			assert_eq!(Balances::balance_frozen(&FreezeReason::Staking.into(), &5), 100);
			assert_eq!(
				UserStake::<Test>::get(5),
				UserStakeInfo {
					stake: 0,
					candidates: bbtreeset![],
					maybe_last_unstake: None,
					maybe_last_reward_session: None,
				}
			);
			assert_ok!(CollatorStaking::stake(
				RuntimeOrigin::signed(5),
				vec![StakeTarget { candidate: 3, stake: 20 }].try_into().unwrap()
			));
			assert_ok!(CollatorStaking::stake(
				RuntimeOrigin::signed(5),
				vec![StakeTarget { candidate: 4, stake: 10 }].try_into().unwrap()
			));
			assert_eq!(
				candidate_list(),
				vec![
					(4, CandidateInfo { stake: 10, stakers: 1 }),
					(3, CandidateInfo { stake: 20, stakers: 1 }),
				]
			);

			assert_eq!(
				UserStake::<Test>::get(5),
				UserStakeInfo {
					stake: 30,
					candidates: bbtreeset![3, 4],
					maybe_last_unstake: None,
					maybe_last_reward_session: Some(0),
				}
			);
			assert_ok!(CollatorStaking::leave_intent(RuntimeOrigin::signed(3)));
			assert_eq!(candidate_list(), vec![(4, CandidateInfo { stake: 10, stakers: 1 })]);

			// the stake should be untouched.
			assert_eq!(
				UserStake::<Test>::get(5),
				UserStakeInfo {
					stake: 30,
					candidates: bbtreeset![3, 4],
					maybe_last_unstake: None,
					maybe_last_reward_session: Some(0),
				}
			);
			assert_eq!(Balances::balance_frozen(&FreezeReason::Staking.into(), &5), 100);
			assert_ok!(CollatorStaking::unstake_all(RuntimeOrigin::signed(5)));
			System::assert_has_event(RuntimeEvent::CollatorStaking(Event::StakeRemoved {
				account: 5,
				candidate: 3,
				amount: 20,
			}));
			System::assert_has_event(RuntimeEvent::CollatorStaking(Event::StakeRemoved {
				account: 5,
				candidate: 4,
				amount: 10,
			}));
			assert_eq!(ReleaseQueues::<Test>::get(5), vec![]);
			assert_eq!(
				CandidateStake::<Test>::get(3, 5),
				CandidateStakeInfo { stake: 0, session: 0 }
			);
			assert_eq!(
				CandidateStake::<Test>::get(4, 5),
				CandidateStakeInfo { stake: 0, session: 0 }
			);
			assert_eq!(
				UserStake::<Test>::get(5),
				UserStakeInfo {
					stake: 0,
					candidates: bbtreeset![],
					// Candidate 3 left, so 20 immediately restakable.
					maybe_last_unstake: Some((10, 11)),
					maybe_last_reward_session: None,
				}
			);
			assert_eq!(Balances::balance_frozen(&FreezeReason::Staking.into(), &5), 100);
			assert_eq!(candidate_list(), vec![(4, CandidateInfo { stake: 0, stakers: 0 })]);
		});
	}

	#[test]
	fn must_claim_before_unstake_all() {
		new_test_ext().execute_with(|| {
			initialize_to_block(1);

			register_candidates(3..=4);
			lock_for_staking(5..=5);
			assert_eq!(
				UserStake::<Test>::get(5),
				UserStakeInfo {
					stake: 0,
					candidates: bbtreeset![],
					maybe_last_unstake: None,
					maybe_last_reward_session: None,
				}
			);
			assert_ok!(CollatorStaking::stake(
				RuntimeOrigin::signed(5),
				vec![StakeTarget { candidate: 3, stake: 20 }].try_into().unwrap()
			));

			// Time travel to the next Session
			initialize_to_block(10);
			assert_eq!(CurrentSession::<Test>::get(), 1);
			assert_noop!(
				CollatorStaking::unstake_all(RuntimeOrigin::signed(5)),
				Error::<Test>::PreviousRewardsNotClaimed
			);

			// Claim and retry operation
			assert_ok!(CollatorStaking::claim_rewards(RuntimeOrigin::signed(5)));
			assert_ok!(CollatorStaking::unstake_all(RuntimeOrigin::signed(5)));
		});
	}
}

mod set_autocompound_percentage {
	use super::*;

	#[test]
	fn set_autocompound_percentage() {
		new_test_ext().execute_with(|| {
			initialize_to_block(1);

			assert_eq!(AutoCompound::<Test>::get(5), Percent::from_parts(0));
			assert_noop!(
				CollatorStaking::set_autocompound_percentage(
					RuntimeOrigin::signed(5),
					Percent::from_parts(50)
				),
				Error::<Test>::InsufficientStake
			);

			lock_for_staking(5..=5);
			assert_ok!(CollatorStaking::set_autocompound_percentage(
				RuntimeOrigin::signed(5),
				Percent::from_parts(50)
			));
			assert_eq!(AutoCompound::<Test>::get(5), Percent::from_parts(50));
			System::assert_last_event(RuntimeEvent::CollatorStaking(
				Event::AutoCompoundPercentageSet {
					account: 5,
					percentage: Percent::from_parts(50),
				},
			));
			// Set it back to zero.
			assert_ok!(CollatorStaking::set_autocompound_percentage(
				RuntimeOrigin::signed(5),
				Percent::from_parts(0)
			));
			assert_eq!(AutoCompound::<Test>::get(5), Percent::from_parts(0));
			System::assert_last_event(RuntimeEvent::CollatorStaking(Event::AutoCompoundDisabled {
				account: 5,
			}));
		});
	}

	#[test]
	fn must_claim_before_set_autocompound_percentage() {
		new_test_ext().execute_with(|| {
			initialize_to_block(1);

			register_candidates(3..=4);
			lock_for_staking(5..=5);
			assert_eq!(
				UserStake::<Test>::get(5),
				UserStakeInfo {
					stake: 0,
					candidates: bbtreeset![],
					maybe_last_unstake: None,
					maybe_last_reward_session: None,
				}
			);
			assert_ok!(CollatorStaking::stake(
				RuntimeOrigin::signed(5),
				vec![StakeTarget { candidate: 3, stake: 20 }].try_into().unwrap()
			));

			// Time travel to the next Session
			initialize_to_block(10);
			assert_eq!(CurrentSession::<Test>::get(), 1);
			assert_noop!(
				CollatorStaking::set_autocompound_percentage(
					RuntimeOrigin::signed(5),
					Percent::from_parts(50)
				),
				Error::<Test>::PreviousRewardsNotClaimed
			);

			// Claim and retry operation
			assert_ok!(CollatorStaking::claim_rewards(RuntimeOrigin::signed(5)));
			assert_ok!(CollatorStaking::set_autocompound_percentage(
				RuntimeOrigin::signed(5),
				Percent::from_parts(50)
			));
		});
	}
}

mod lock_unlock_and_release {
	use super::*;

	#[test]
	fn lock() {
		new_test_ext().execute_with(|| {
			initialize_to_block(1);

			assert_eq!(Balances::balance(&5), 100);
			assert_eq!(Balances::balance_frozen(&FreezeReason::Staking.into(), &5), 0);
			assert_ok!(CollatorStaking::lock(RuntimeOrigin::signed(5), 60));
			System::assert_last_event(RuntimeEvent::CollatorStaking(Event::LockExtended {
				account: 5,
				amount: 60,
			}));
			assert_eq!(Balances::balance(&5), 100);
			assert_eq!(Balances::balance_frozen(&FreezeReason::Staking.into(), &5), 60);

			// we cannot lock over the balance
			assert_eq!(CollatorStaking::get_free_balance(&5), 40);
			assert_noop!(
				CollatorStaking::lock(RuntimeOrigin::signed(5), 50),
				Error::<Test>::InsufficientFreeBalance
			);
		});
	}

	#[test]
	fn unlock() {
		new_test_ext().execute_with(|| {
			initialize_to_block(1);

			assert_eq!(Balances::balance(&5), 100);
			assert_eq!(Balances::balance_frozen(&FreezeReason::Staking.into(), &5), 0);
			assert_ok!(CollatorStaking::lock(RuntimeOrigin::signed(5), 60));
			System::assert_last_event(RuntimeEvent::CollatorStaking(Event::LockExtended {
				account: 5,
				amount: 60,
			}));
			assert_eq!(Balances::balance(&5), 100);
			assert_eq!(Balances::balance_frozen(&FreezeReason::Staking.into(), &5), 60);
			assert_eq!(CollatorStaking::get_free_balance(&5), 40);

			// We have now enough balance to be able to enable autocompounding
			assert_ok!(CollatorStaking::set_autocompound_percentage(
				RuntimeOrigin::signed(5),
				Percent::from_parts(50),
			));

			// we cannot unlock more funds than what we currently have
			assert_noop!(
				CollatorStaking::unlock(RuntimeOrigin::signed(5), Some(100)),
				Error::<Test>::CannotUnlock
			);
			register_candidates(4..=4);
			assert_ok!(CollatorStaking::stake(
				RuntimeOrigin::signed(5),
				vec![StakeTarget { candidate: 4, stake: 50 }].try_into().unwrap()
			));
			// we now only have 10 locked but not staked
			assert_noop!(
				CollatorStaking::unlock(RuntimeOrigin::signed(5), Some(20)),
				Error::<Test>::CannotUnlock
			);
			assert_ok!(CollatorStaking::unlock(RuntimeOrigin::signed(5), None));
			assert_eq!(Balances::balance_frozen(&FreezeReason::Staking.into(), &5), 50);
			assert_eq!(Balances::balance_frozen(&FreezeReason::Releasing.into(), &5), 10);

			// If reducing the staked balance under the threshold there should be an event
			System::assert_has_event(RuntimeEvent::CollatorStaking(Event::AutoCompoundDisabled {
				account: 5,
			}));
		});
	}

	#[test]
	fn claim_with_empty_list() {
		new_test_ext().execute_with(|| {
			initialize_to_block(1);

			assert_eq!(System::events(), vec![]);
			assert_eq!(ReleaseQueues::<Test>::get(5), vec![]);
			assert_ok!(CollatorStaking::release(RuntimeOrigin::signed(5)));
			assert_eq!(System::events(), vec![]);
			assert_eq!(ReleaseQueues::<Test>::get(5), vec![]);
		});
	}

	#[test]
	fn claim() {
		new_test_ext().execute_with(|| {
			initialize_to_block(1);

			lock_for_staking(5..=5);
			assert_eq!(Balances::balance_frozen(&FreezeReason::Staking.into(), &5), 100);
			assert_ok!(CollatorStaking::unlock(RuntimeOrigin::signed(5), Some(20)));
			assert_eq!(Balances::balance_frozen(&FreezeReason::Staking.into(), &5), 80);
			assert_eq!(Balances::balance_frozen(&FreezeReason::Releasing.into(), &5), 20);
			System::assert_last_event(RuntimeEvent::CollatorStaking(
				Event::ReleaseRequestCreated { account: 5, amount: 20, block: 3 },
			));
			// No changes until delay passes
			assert_eq!(
				ReleaseQueues::<Test>::get(5),
				vec![ReleaseRequest { block: 3, amount: 20 }]
			);
			assert_ok!(CollatorStaking::release(RuntimeOrigin::signed(5)));
			assert_eq!(
				ReleaseQueues::<Test>::get(5),
				vec![ReleaseRequest { block: 3, amount: 20 }]
			);

			initialize_to_block(3);
			assert_ok!(CollatorStaking::release(RuntimeOrigin::signed(5)));
			assert_eq!(ReleaseQueues::<Test>::get(5), vec![]);
			System::assert_last_event(RuntimeEvent::CollatorStaking(Event::StakeReleased {
				account: 5,
				amount: 20,
			}));
		});
	}

	#[test]
	fn lock_stake_unstake_unlock() {
		new_test_ext().execute_with(|| {
			initialize_to_block(1);
			register_candidates(4..=4);

			// Lock 20 tokens for account 5
			assert_eq!(Balances::balance_frozen(&FreezeReason::Staking.into(), &5), 0);
			assert_ok!(CollatorStaking::lock(RuntimeOrigin::signed(5), 20));
			System::assert_last_event(RuntimeEvent::CollatorStaking(Event::LockExtended {
				account: 5,
				amount: 20,
			}));
			assert_eq!(Balances::balance_frozen(&FreezeReason::Staking.into(), &5), 20);
			assert_eq!(
				UserStake::<Test>::get(5),
				UserStakeInfo {
					stake: 0,
					candidates: bbtreeset![],
					maybe_last_unstake: None,
					maybe_last_reward_session: None,
				}
			);

			// Stake the tokens
			assert_ok!(CollatorStaking::stake(
				RuntimeOrigin::signed(5),
				vec![StakeTarget { candidate: 4, stake: 20 }].try_into().unwrap()
			));
			assert_eq!(Balances::balance_frozen(&FreezeReason::Staking.into(), &5), 20);
			assert_eq!(
				UserStake::<Test>::get(5),
				UserStakeInfo {
					stake: 20,
					candidates: bbtreeset![4],
					maybe_last_unstake: None,
					maybe_last_reward_session: Some(0),
				}
			);

			// Unstake the tokens
			assert_ok!(CollatorStaking::unstake_from(RuntimeOrigin::signed(5), 4));
			assert_eq!(Balances::balance_frozen(&FreezeReason::Staking.into(), &5), 20);
			assert_eq!(Balances::balance_frozen(&FreezeReason::Releasing.into(), &5), 0);
			assert_eq!(
				UserStake::<Test>::get(5),
				UserStakeInfo {
					stake: 0,
					candidates: bbtreeset![],
					maybe_last_unstake: Some((20, 11)),
					maybe_last_reward_session: None,
				}
			);

			// Attempt first 10 tokens unlock
			assert_ok!(CollatorStaking::unlock(RuntimeOrigin::signed(5), Some(10)));
			assert_eq!(Balances::balance_frozen(&FreezeReason::Staking.into(), &5), 10);
			assert_eq!(Balances::balance_frozen(&FreezeReason::Releasing.into(), &5), 10);
			assert_eq!(
				UserStake::<Test>::get(5),
				UserStakeInfo {
					stake: 0,
					candidates: bbtreeset![],
					maybe_last_unstake: Some((10, 11)),
					maybe_last_reward_session: None,
				}
			);

			// Attempt second 10 tokens unlock
			assert_ok!(CollatorStaking::unlock(RuntimeOrigin::signed(5), Some(10)));
			assert_eq!(Balances::balance_frozen(&FreezeReason::Staking.into(), &5), 0);
			assert_eq!(Balances::balance_frozen(&FreezeReason::Releasing.into(), &5), 20);
			assert_eq!(
				UserStake::<Test>::get(5),
				UserStakeInfo {
					stake: 0,
					candidates: bbtreeset![],
					maybe_last_unstake: None,
					maybe_last_reward_session: None,
				}
			);
		});
	}

	#[test]
	fn too_many_release_requests() {
		new_test_ext().execute_with(|| {
			initialize_to_block(1);

			// Preconditions
			assert_eq!(<Test as Config>::MaxStakedCandidates::get(), 16);

			// Lock tokens for account 5
			lock_for_staking(5..=5);
			assert_eq!(Balances::balance_frozen(&FreezeReason::Staking.into(), &5), 100);

			// Try to create release requests up to the maximum allowed
			for i in 1..=16 {
				assert_ok!(CollatorStaking::unlock(RuntimeOrigin::signed(5), Some(1)));
				let expected_queue =
					(1..=i).map(|_| ReleaseRequest { block: 3, amount: 1 }).collect::<Vec<_>>();
				assert_eq!(ReleaseQueues::<Test>::get(5), expected_queue);
			}

			// Attempting one more release request should raise an error
			assert_noop!(
				CollatorStaking::unlock(RuntimeOrigin::signed(5), Some(1)),
				Error::<Test>::TooManyReleaseRequests
			);

			// Ensure no additional request was added
			assert_eq!(ReleaseQueues::<Test>::get(5).len(), 16);
		});
	}
}

mod set_collator_reward_percentage {
	use super::*;

	#[test]
	fn set_collator_reward_percentage() {
		new_test_ext().execute_with(|| {
			initialize_to_block(1);

			assert_eq!(CollatorRewardPercentage::<Test>::get(), Percent::from_parts(20));

			// Invalid origin
			assert_noop!(
				CollatorStaking::set_collator_reward_percentage(
					RuntimeOrigin::signed(5),
					Percent::from_parts(50)
				),
				BadOrigin
			);
			assert_ok!(CollatorStaking::set_collator_reward_percentage(
				RuntimeOrigin::signed(RootAccount::get()),
				Percent::from_parts(50)
			));
			System::assert_last_event(RuntimeEvent::CollatorStaking(
				Event::CollatorRewardPercentageSet { percentage: Percent::from_parts(50) },
			));
			assert_eq!(CollatorRewardPercentage::<Test>::get(), Percent::from_parts(50));
		});
	}
}

mod set_extra_reward {
	use super::*;

	#[test]
	fn set_extra_reward() {
		new_test_ext().execute_with(|| {
			initialize_to_block(1);

			assert_eq!(ExtraReward::<Test>::get(), 0);

			// Invalid origin
			assert_noop!(
				CollatorStaking::set_extra_reward(RuntimeOrigin::signed(5), 10),
				BadOrigin
			);

			// Set the reward
			assert_ok!(CollatorStaking::set_extra_reward(
				RuntimeOrigin::signed(RootAccount::get()),
				10
			));
			System::assert_last_event(RuntimeEvent::CollatorStaking(Event::ExtraRewardSet {
				amount: 10,
			}));
			assert_eq!(ExtraReward::<Test>::get(), 10);

			// Cannot set to zero
			assert_noop!(
				CollatorStaking::set_extra_reward(RuntimeOrigin::signed(RootAccount::get()), 0),
				Error::<Test>::InvalidExtraReward
			);

			// Revert the changes
			assert_ok!(CollatorStaking::stop_extra_reward(RuntimeOrigin::signed(
				RootAccount::get()
			),));
			System::assert_last_event(RuntimeEvent::CollatorStaking(Event::ExtraRewardRemoved {
				amount_left: 0,
				receiver: Some(40),
			}));
			assert_eq!(ExtraReward::<Test>::get(), 0);
		});
	}
}

mod set_minimum_stake {
	use super::*;

	#[test]
	fn set_minimum_stake() {
		new_test_ext().execute_with(|| {
			initialize_to_block(1);

			assert_eq!(MinStake::<Test>::get(), 2);

			// Invalid origin
			assert_noop!(
				CollatorStaking::set_minimum_stake(RuntimeOrigin::signed(5), 5),
				BadOrigin
			);

			// Set the reward over CandidacyBond
			assert_noop!(
				CollatorStaking::set_minimum_stake(RuntimeOrigin::signed(RootAccount::get()), 1000),
				Error::<Test>::InvalidMinStake
			);

			// Zero is a valid value
			assert_ok!(CollatorStaking::set_minimum_stake(
				RuntimeOrigin::signed(RootAccount::get()),
				0
			));
			System::assert_last_event(RuntimeEvent::CollatorStaking(Event::NewMinStake {
				min_stake: 0,
			}));
			assert_eq!(MinStake::<Test>::get(), 0);

			// Maximum is CandidacyBond
			assert_eq!(MinCandidacyBond::<Test>::get(), 10);
			assert_ok!(CollatorStaking::set_minimum_stake(
				RuntimeOrigin::signed(RootAccount::get()),
				10
			));
			System::assert_last_event(RuntimeEvent::CollatorStaking(Event::NewMinStake {
				min_stake: 10,
			}));
			assert_eq!(MinStake::<Test>::get(), 10);
		});
	}
}

mod top_up_extra_rewards {
	use super::*;

	#[test]
	fn top_up_extra_rewards() {
		new_test_ext().execute_with(|| {
			initialize_to_block(1);

			assert_eq!(Balances::balance(&CollatorStaking::extra_reward_account_id()), 0);

			// Cannot fund with an amount equal to zero.
			assert_noop!(
				CollatorStaking::top_up_extra_rewards(RuntimeOrigin::signed(1), 0),
				Error::<Test>::InvalidFundingAmount
			);

			// Cannot fund if total balance less than ED.
			assert_noop!(
				CollatorStaking::top_up_extra_rewards(RuntimeOrigin::signed(1), 1),
				TokenError::BelowMinimum
			);

			// Now we can top it up.
			assert_ok!(CollatorStaking::top_up_extra_rewards(RuntimeOrigin::signed(1), 10));

			System::assert_last_event(RuntimeEvent::CollatorStaking(Event::ExtraRewardPotFunded {
				pot: CollatorStaking::extra_reward_account_id(),
				amount: 10,
			}));
			assert_eq!(Balances::balance(&CollatorStaking::extra_reward_account_id()), 10);
		});
	}
}

mod update_candidacy_bond {
	use super::*;

	#[test]
	fn update_candidacy_bond() {
		new_test_ext().execute_with(|| {
			initialize_to_block(1);

			register_candidates(3..=3);
			assert_eq!(Balances::balance_frozen(&FreezeReason::CandidacyBond.into(), &3), 10);

			// Cannot set it below the minimum candidacy bond.
			assert_noop!(
				CollatorStaking::update_candidacy_bond(RuntimeOrigin::signed(3), 5),
				Error::<Test>::InvalidCandidacyBond
			);
			// Cannot set it if not candidate.
			assert_noop!(
				CollatorStaking::update_candidacy_bond(RuntimeOrigin::signed(4), 15),
				Error::<Test>::NotCandidate
			);
			// Cannot set it not enough free balance.
			assert_noop!(
				CollatorStaking::update_candidacy_bond(
					RuntimeOrigin::signed(3),
					Balances::balance(&3) + 10
				),
				Error::<Test>::InsufficientFreeBalance
			);

			assert_ok!(CollatorStaking::update_candidacy_bond(RuntimeOrigin::signed(3), 20));
			System::assert_last_event(RuntimeEvent::CollatorStaking(
				Event::<Test>::CandidacyBondUpdated { candidate: 3, new_bond: 20 },
			));
			assert_eq!(Balances::balance_frozen(&FreezeReason::CandidacyBond.into(), &3), 20);
		});
	}
}

mod general_tests {
	use super::*;

	#[test]
	fn basic_setup_works() {
		new_test_ext().execute_with(|| {
			initialize_to_block(1);

			assert_eq!(<Test as Config>::MaxInvulnerables::get(), 20);
			assert_eq!(<Test as Config>::MaxCandidates::get(), 20);
			assert_eq!(<Test as Config>::MinEligibleCollators::get(), 1);
			assert_eq!(<Test as Config>::KickThreshold::get(), 10);
			assert_eq!(<Test as Config>::MaxStakedCandidates::get(), 16);
			assert_eq!(<Test as Config>::BondUnlockDelay::get(), 5);
			assert_eq!(<Test as Config>::StakeUnlockDelay::get(), 2);
			assert_eq!(<Test as Config>::MaxStakers::get(), 25);

			assert_eq!(DesiredCandidates::<Test>::get(), 2);
			assert_eq!(MinCandidacyBond::<Test>::get(), 10);
			assert_eq!(MinStake::<Test>::get(), 2);
			assert_eq!(Candidates::<Test>::count(), 0);
			assert_eq!(CollatorRewardPercentage::<Test>::get(), Percent::from_parts(20));
			// The minimum balance should not have been minted
			assert_eq!(Balances::balance(&CollatorStaking::account_id()), 0);
			// genesis should sort input
			assert_eq!(Invulnerables::<Test>::get(), vec![1, 2]);

			#[cfg(feature = "try-runtime")]
			{
				use frame_system::pallet_prelude::BlockNumberFor;
				assert_ok!(<CollatorStaking as frame_support::traits::Hooks<
					BlockNumberFor<Test>,
				>>::try_state(1));
			}
		});
	}

	#[test]
	fn candidate_list_works() {
		new_test_ext().execute_with(|| {
			initialize_to_block(1);

			// given
			assert_eq!(DesiredCandidates::<Test>::get(), 2);
			assert_eq!(MinCandidacyBond::<Test>::get(), 10);

			assert_eq!(Candidates::<Test>::count(), 0);
			assert_eq!(Invulnerables::<Test>::get(), vec![1, 2]);

			// take three endowed, non-invulnerables accounts.
			assert_eq!(Balances::balance(&3), 100);
			assert_eq!(
				CandidateStake::<Test>::get(3, 3),
				CandidateStakeInfo { stake: 0, session: 0 }
			);
			assert_eq!(Balances::balance(&4), 100);
			assert_eq!(
				CandidateStake::<Test>::get(4, 4),
				CandidateStakeInfo { stake: 0, session: 0 }
			);
			assert_eq!(Balances::balance(&5), 100);
			assert_eq!(
				CandidateStake::<Test>::get(5, 5),
				CandidateStakeInfo { stake: 0, session: 0 }
			);
			register_candidates(3..=5);
			lock_for_staking(3..=5);

			assert_ok!(CollatorStaking::stake(
				RuntimeOrigin::signed(5),
				vec![StakeTarget { candidate: 5, stake: 20 }].try_into().unwrap()
			));
			assert_ok!(CollatorStaking::stake(
				RuntimeOrigin::signed(3),
				vec![StakeTarget { candidate: 3, stake: 30 }].try_into().unwrap()
			));
			assert_ok!(CollatorStaking::stake(
				RuntimeOrigin::signed(4),
				vec![StakeTarget { candidate: 4, stake: 25 }].try_into().unwrap()
			));
			assert_ok!(CollatorStaking::stake(
				RuntimeOrigin::signed(5),
				vec![StakeTarget { candidate: 5, stake: 30 }].try_into().unwrap()
			));

			let candidate_3 = CandidateInfo { stake: 30, stakers: 1 };
			let candidate_4 = CandidateInfo { stake: 25, stakers: 1 };
			let candidate_5 = CandidateInfo { stake: 50, stakers: 1 };
			assert_eq!(
				candidate_list(),
				vec![(4, candidate_4), (3, candidate_3), (5, candidate_5)]
			);
		});
	}

	#[test]
	fn fees_edgecases() {
		new_test_ext().execute_with(|| {
			initialize_to_block(1);

			Balances::mint_into(&CollatorStaking::account_id(), Balances::minimum_balance())
				.unwrap();

			// Nothing panics, no reward when no ED in balance
			Authorship::on_initialize(1);
			// 4 is the default author.
			assert_eq!(Balances::balance(&4), 100);
			register_candidates(4..=4);
			// triggers `note_author`
			Authorship::on_initialize(1);

			// tuple of (id, deposit).
			let collator = CandidateInfo { stake: 0, stakers: 0 };

			assert_eq!(candidate_list(), vec![(4, collator)]);
			assert_eq!(LastAuthoredBlock::<Test>::get(4), 1);
			// Nothing received
			assert_eq!(Balances::balance_frozen(&FreezeReason::CandidacyBond.into(), &4), 10);
			// all fee stays
			assert_eq!(Balances::balance(&CollatorStaking::account_id()), 5);
		});
	}

	#[test]
	#[should_panic = "duplicate invulnerables in genesis."]
	fn cannot_set_genesis_value_twice() {
		sp_tracing::try_init_simple();
		let mut t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();
		let invulnerables = vec![1, 1];

		let collator_staking = collator_staking::GenesisConfig::<Test> {
			desired_candidates: 2,
			min_candidacy_bond: 10,
			min_stake: 1,
			invulnerables,
			collator_reward_percentage: Percent::from_parts(20),
			extra_reward: 0,
		};
		// collator selection must be initialized before session.
		collator_staking.assimilate_storage(&mut t).unwrap();
	}

	#[test]
	#[should_panic = "genesis desired_candidates are more than T::MaxCandidates"]
	fn cannot_set_invalid_max_candidates_in_genesis() {
		sp_tracing::try_init_simple();
		let mut t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();

		let collator_staking = collator_staking::GenesisConfig::<Test> {
			desired_candidates: 50,
			min_candidacy_bond: 10,
			min_stake: 2,
			invulnerables: vec![1, 2],
			collator_reward_percentage: Percent::from_parts(20),
			extra_reward: 0,
		};
		// collator selection must be initialized before session.
		collator_staking.assimilate_storage(&mut t).unwrap();
	}

	#[test]
	#[should_panic = "genesis invulnerables are more than T::MaxInvulnerables"]
	fn cannot_set_too_many_invulnerables_at_genesis() {
		sp_tracing::try_init_simple();
		let mut t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();

		let collator_staking = collator_staking::GenesisConfig::<Test> {
			desired_candidates: 5,
			min_candidacy_bond: 10,
			min_stake: 2,
			invulnerables: vec![
				1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21,
			],
			collator_reward_percentage: Percent::from_parts(20),
			extra_reward: 0,
		};
		// collator selection must be initialized before session.
		collator_staking.assimilate_storage(&mut t).unwrap();
	}
}

mod collator_rewards {
	use super::*;

	#[test]
	fn should_not_reward_invulnerables() {
		new_test_ext().execute_with(|| {
			initialize_to_block(1);

			assert_ok!(CollatorStaking::add_invulnerable(
				RuntimeOrigin::signed(RootAccount::get()),
				4
			));
			assert_eq!(ExtraReward::<Test>::get(), 0);
			assert_eq!(TotalBlocks::<Test>::get(), (1, 1));
			assert_eq!(CurrentSession::<Test>::get(), 0);
			for block in 1..=9 {
				initialize_to_block(block);
				assert_eq!(CurrentSession::<Test>::get(), 0);
				assert_eq!(TotalBlocks::<Test>::get(), (block as u32, 1));

				// Transfer the ED first
				Balances::mint_into(&CollatorStaking::account_id(), Balances::minimum_balance())
					.unwrap();

				// Assume we collected one unit in fees per block
				assert_ok!(Balances::transfer(&1, &CollatorStaking::account_id(), 1, Preserve));
			}

			assert_eq!(ProducedBlocks::<Test>::get(4), 1);
			initialize_to_block(10);
			assert_eq!(CurrentSession::<Test>::get(), 1);
			assert_eq!(TotalBlocks::<Test>::get(), (1, 0));

			// No StakingRewardReceived should have been emitted if only invulnerable is producing blocks.
			assert!(!System::events().iter().any(|e| {
				matches!(
					e.event,
					RuntimeEvent::CollatorStaking(Event::StakingRewardReceived { .. })
				)
			}));
		});
	}

	#[test]
	fn should_reward_collator() {
		new_test_ext().execute_with(|| {
			initialize_to_block(1);

			assert_ok!(CollatorStaking::register_as_candidate(
				RuntimeOrigin::signed(4),
				MinCandidacyBond::<Test>::get()
			));
			lock_for_staking(4..=4);
			assert_ok!(CollatorStaking::stake(
				RuntimeOrigin::signed(4),
				vec![StakeTarget { candidate: 4, stake: 10 }].try_into().unwrap()
			));
			assert_eq!(ExtraReward::<Test>::get(), 0);
			assert_eq!(Balances::balance(&CollatorStaking::account_id()), 0);
			Balances::mint_into(&CollatorStaking::account_id(), Balances::minimum_balance())
				.unwrap();
			assert_eq!(TotalBlocks::<Test>::get(), (1, 1));
			assert_eq!(CurrentSession::<Test>::get(), 0);
			assert_eq!(CollatorStaking::calculate_unclaimed_rewards(&4), 0);
			for block in 1..=9 {
				initialize_to_block(block);
				assert_eq!(CurrentSession::<Test>::get(), 0);
				assert_eq!(TotalBlocks::<Test>::get(), (block as u32, block as u32));

				// Assume we collected one unit in fees per block
				assert_ok!(Balances::transfer(&1, &CollatorStaking::account_id(), 1, Preserve));
			}
			assert_eq!(CollatorStaking::calculate_unclaimed_rewards(&4), 0);
			assert_eq!(
				Balances::balance(&CollatorStaking::account_id()),
				Balances::minimum_balance() + 9
			);
			assert!(!System::events().iter().any(|e| {
				matches!(
					e.event,
					RuntimeEvent::CollatorStaking(Event::StakingRewardReceived { .. })
				)
			}));

			assert_eq!(ProducedBlocks::<Test>::get(4), 9);
			assert_eq!(ClaimableRewards::<Test>::get(), 0);
			initialize_to_block(10);
			assert_eq!(CollatorStaking::calculate_unclaimed_rewards(&4), 0);
			assert_eq!(CurrentSession::<Test>::get(), 1);
			assert_eq!(TotalBlocks::<Test>::get(), (1, 1));
			assert_eq!(ProducedBlocks::<Test>::get(4), 1);
			// No rewards for stakers in session zero!
			assert_eq!(ClaimableRewards::<Test>::get(), 0);

			System::assert_has_event(RuntimeEvent::CollatorStaking(Event::StakingRewardReceived {
				account: 4,
				amount: 1,
			}));

			System::reset_events();

			for block in 10..=19 {
				initialize_to_block(block);
				assert_eq!(CurrentSession::<Test>::get(), 1);
				assert_eq!(TotalBlocks::<Test>::get(), (block as u32 - 9, block as u32 - 9));

				// Assume we collected one unit in fees per block
				assert_ok!(Balances::transfer(&1, &CollatorStaking::account_id(), 1, Preserve));
			}

			assert_eq!(CollatorStaking::calculate_unclaimed_rewards(&4), 0);
			assert_eq!(
				Balances::free_balance(CollatorStaking::account_id()) - Balances::minimum_balance(),
				18
			);
			// we can safely remove the collator, as rewards will be delivered anyway to both
			// the collator itself and its stakers.
			assert_ok!(CollatorStaking::leave_intent(RuntimeOrigin::signed(4)));

			initialize_to_block(20);
			System::assert_has_event(RuntimeEvent::CollatorStaking(Event::SessionEnded {
				index: 1,
				rewards: 18,
			}));
			// Rationale: the pot had 18 for rewards + 5 of existential deposit, and rewarded immediately
			// 3 for the collator (20% of the total rewards), so 20 in the end.
			assert_eq!(Balances::balance(&CollatorStaking::account_id()), 20);
			assert_eq!(CurrentSession::<Test>::get(), 2);
			assert_eq!(TotalBlocks::<Test>::get(), (1, 1));

			// the block 20 just got produced, and belongs to the new session.
			assert_eq!(ProducedBlocks::<Test>::get(4), 1);

			// Total rewards in session 1: 18 (8 accumulated from session 0)
			// 3 (20%) for collators and paid immediately.
			// 15 (80%) for stakers and payment delayed until `claim_rewards` is called.

			// Reward for collator.
			System::assert_has_event(RuntimeEvent::CollatorStaking(Event::StakingRewardReceived {
				account: 4,
				amount: 3,
			}));
			// No rewards for stakers until claimed.
			assert!(!System::events().iter().any(|e| {
				matches!(
					e.event,
					RuntimeEvent::CollatorStaking(Event::StakingRewardReceived {
						account: 4,
						amount: 15,
					})
				)
			}));
			assert_eq!(CollatorStaking::calculate_unclaimed_rewards(&4), 15);
			assert_ok!(CollatorStaking::claim_rewards(RuntimeOrigin::signed(4)));
			assert_eq!(ClaimableRewards::<Test>::get(), 0);
			// Now we can see the reward.
			System::assert_has_event(RuntimeEvent::CollatorStaking(Event::StakingRewardReceived {
				account: 4,
				amount: 15,
			}));
		});
	}

	#[test]
	fn should_remove_oldest_reward() {
		new_test_ext().execute_with(|| {
			initialize_to_block(1);

			assert_eq!(<Test as Config>::MaxRewardSessions::get(), 10);
			register_candidates(3..=3);
			lock_for_staking(3..=3);
			assert_ok!(CollatorStaking::stake(
				RuntimeOrigin::signed(3),
				vec![StakeTarget { candidate: 3, stake: 60 }].try_into().unwrap()
			));
			assert!(PerSessionRewards::<Test>::get(0).is_none());

			assert_eq!(PerSessionRewards::<Test>::count(), 0);
			assert!(PerSessionRewards::<Test>::get(0).is_none());
			// We want to discard the first session where there are no rewards.
			initialize_to_block(10);
			assert_eq!(PerSessionRewards::<Test>::count(), 1);
			assert!(matches!(
				PerSessionRewards::<Test>::get(0),
				Some(SessionInfo { rewards: 0, claimed_rewards: 0, .. })
			));
			assert!(PerSessionRewards::<Test>::get(0).is_some());
			// Fund the pot with the ED to make calculations easier.
			assert_ok!(Balances::mint_into(
				&CollatorStaking::account_id(),
				Balances::minimum_balance()
			));
			assert_eq!(CollatorStaking::calculate_unclaimed_rewards(&3), 0);

			for session in 2..=10 {
				assert_ok!(Balances::mint_into(&CollatorStaking::account_id(), 100));
				ProducedBlocks::<Test>::insert(3, 10);
				let current_block = (session * 10) as u64;
				LastAuthoredBlock::<Test>::insert(3, current_block);
				initialize_to_block(current_block);
				assert_eq!(PerSessionRewards::<Test>::count(), session);
				let claimable_rewards = 80 * (session - 1) as u64;
				assert_eq!(ClaimableRewards::<Test>::get(), claimable_rewards);
				assert_eq!(CollatorStaking::calculate_unclaimed_rewards(&3), claimable_rewards);
				assert!(matches!(
					PerSessionRewards::<Test>::get(session - 1),
					Some(SessionInfo { rewards: 80, claimed_rewards: 0, .. })
				));
			}
			assert_eq!(CollatorStaking::current_block_number(), 100);
			assert_eq!(PerSessionRewards::<Test>::count(), 10);
			assert_eq!(ClaimableRewards::<Test>::get(), 720);
			assert_eq!(CollatorStaking::calculate_unclaimed_rewards(&3), 720);

			// So far no rewards should have been removed, so 80 * 9 = 720
			assert_eq!(CollatorStaking::calculate_unclaimed_rewards(&3), 720);
			assert!(PerSessionRewards::<Test>::get(0).is_some());
			assert!(PerSessionRewards::<Test>::get(1).is_some());
			LastAuthoredBlock::<Test>::insert(3, 110);

			// Rewards for session 0 should be removed.
			ProducedBlocks::<Test>::insert(3, 10);
			initialize_to_block(110);
			assert_eq!(CollatorStaking::current_block_number(), 110);
			assert_eq!(PerSessionRewards::<Test>::count(), 10); // this must not increase
			assert!(PerSessionRewards::<Test>::get(0).is_none());
			assert!(PerSessionRewards::<Test>::get(1).is_some());
			LastAuthoredBlock::<Test>::insert(3, 120);
			assert_eq!(ClaimableRewards::<Test>::get(), 720);
			// This is important: since the staker did not claim rewards for session zero have disappeared now.
			// However, there are no rewards for stakers for session zero, so they remain the same.
			assert_eq!(CollatorStaking::calculate_unclaimed_rewards(&3), 720);

			// Rewards for session 1 should be removed.
			ProducedBlocks::<Test>::insert(3, 10);
			initialize_to_block(120);
			assert_eq!(CollatorStaking::current_block_number(), 120);
			assert_eq!(PerSessionRewards::<Test>::count(), 10);
			assert!(PerSessionRewards::<Test>::get(1).is_none());
			assert!(PerSessionRewards::<Test>::get(2).is_some());
			LastAuthoredBlock::<Test>::insert(3, 130);
			// Now rewards for session one have disappeared and the user lost the right to claim them.
			// However, 80 were lost, but readded to the pot. So 20% of those 80 (16) were used to reward
			// the collator, leaving 64 that go again to the same collator as staking rewards, leaving
			// a total of 704.
			assert_eq!(ClaimableRewards::<Test>::get(), 704);
			assert_eq!(CollatorStaking::calculate_unclaimed_rewards(&3), 704);

			// Rewards for session 2 should be removed.
			ProducedBlocks::<Test>::insert(3, 10);
			initialize_to_block(130);
			assert_eq!(CollatorStaking::current_block_number(), 130);
			assert_eq!(PerSessionRewards::<Test>::count(), 10);
			assert!(PerSessionRewards::<Test>::get(2).is_none());
			assert!(PerSessionRewards::<Test>::get(3).is_some());
			// And now rewards for session two are lost too. Same math applies here:
			// 704 - 80 + 80 * 0.8 = 688
			assert_eq!(ClaimableRewards::<Test>::get(), 688);
			assert_eq!(CollatorStaking::calculate_unclaimed_rewards(&3), 688);

			// Now we collect the rewards for staker 3.
			assert_ok!(CollatorStaking::claim_rewards(RuntimeOrigin::signed(3)));
			assert_eq!(ClaimableRewards::<Test>::get(), 0);
			assert_eq!(CollatorStaking::calculate_unclaimed_rewards(&3), 0);
		});
	}

	#[test]
	fn should_reward_collator_with_extra_rewards() {
		new_test_ext().execute_with(|| {
			initialize_to_block(1);

			assert_ok!(CollatorStaking::register_as_candidate(
				RuntimeOrigin::signed(4),
				MinCandidacyBond::<Test>::get()
			));
			lock_for_staking(4..=4);
			assert_ok!(CollatorStaking::stake(
				RuntimeOrigin::signed(4),
				vec![StakeTarget { candidate: 4, stake: 10 }].try_into().unwrap()
			));
			ExtraReward::<Test>::set(1);
			assert_eq!(Balances::balance(&CollatorStaking::account_id()), 0);
			Balances::mint_into(&CollatorStaking::account_id(), Balances::minimum_balance())
				.unwrap();
			fund_account(CollatorStaking::extra_reward_account_id());

			assert_eq!(TotalBlocks::<Test>::get(), (1, 1));
			assert_eq!(CurrentSession::<Test>::get(), 0);
			for block in 1..=9 {
				initialize_to_block(block);
				assert_eq!(CurrentSession::<Test>::get(), 0);
				assert_eq!(TotalBlocks::<Test>::get(), (block as u32, block as u32));

				// Assume we collected one unit in fees per block
				assert_ok!(Balances::transfer(&1, &CollatorStaking::account_id(), 1, Preserve));
			}
			assert_eq!(
				Balances::balance(&CollatorStaking::account_id()),
				Balances::minimum_balance() + 9
			);
			assert!(!System::events().iter().any(|e| {
				matches!(
					e.event,
					RuntimeEvent::CollatorStaking(Event::StakingRewardReceived { .. })
				)
			}));

			assert_eq!(ProducedBlocks::<Test>::get(4), 9);
			initialize_to_block(10);
			assert_eq!(CurrentSession::<Test>::get(), 1);
			assert_eq!(TotalBlocks::<Test>::get(), (1, 1));

			assert_eq!(ProducedBlocks::<Test>::get(4), 1);

			// We collected 1 per block, plus 1 as extra reward per block.
			System::assert_has_event(RuntimeEvent::CollatorStaking(Event::StakingRewardReceived {
				account: 4,
				amount: 3,
			}));

			System::reset_events();

			for block in 10..=19 {
				initialize_to_block(block);
				assert_eq!(CurrentSession::<Test>::get(), 1);
				assert_eq!(TotalBlocks::<Test>::get(), (block as u32 - 9, block as u32 - 9));

				// Assume we collected one unit in fees per block.
				assert_ok!(Balances::transfer(&1, &CollatorStaking::account_id(), 1, Preserve));
			}

			assert_eq!(
				Balances::free_balance(CollatorStaking::account_id()) - Balances::minimum_balance(),
				25
			);
			initialize_to_block(20);
			System::assert_has_event(RuntimeEvent::CollatorStaking(Event::SessionEnded {
				index: 1,
				rewards: 35,
			}));
			// 35 was distributed in the rewards, but 7 (20%) went for the collators.
			assert_eq!(
				Balances::free_balance(CollatorStaking::account_id()) - Balances::minimum_balance(),
				28
			);
			assert_eq!(CurrentSession::<Test>::get(), 2);
			assert_eq!(TotalBlocks::<Test>::get(), (1, 1));

			// Block 20 was produced.
			assert_eq!(ProducedBlocks::<Test>::get(4), 1);

			// Total rewards: 25 (accumulated in the pot) + 10 (extra rewards)
			// 3 (20%) for collators
			// 13 (80%) for stakers

			// Reward for collator
			System::assert_has_event(RuntimeEvent::CollatorStaking(Event::StakingRewardReceived {
				account: 4,
				amount: 7,
			}));
			// Reward for staker when claiming.
			assert_eq!(CollatorStaking::calculate_unclaimed_rewards(&4), 28);
			assert_ok!(CollatorStaking::claim_rewards(RuntimeOrigin::signed(4)));
			assert_eq!(ClaimableRewards::<Test>::get(), 0);
			System::assert_has_event(RuntimeEvent::CollatorStaking(Event::StakingRewardReceived {
				account: 4,
				amount: 28,
			}));
		});
	}

	#[test]
	fn should_reward_collator_with_extra_rewards_and_no_funds() {
		new_test_ext().execute_with(|| {
			initialize_to_block(1);

			assert_ok!(CollatorStaking::register_as_candidate(
				RuntimeOrigin::signed(4),
				MinCandidacyBond::<Test>::get()
			));
			lock_for_staking(4..=4);
			assert_ok!(CollatorStaking::stake(
				RuntimeOrigin::signed(4),
				vec![StakeTarget { candidate: 4, stake: 10 }].try_into().unwrap()
			));
			// This account has no funds
			ExtraReward::<Test>::set(1);
			assert_eq!(Balances::balance(&CollatorStaking::account_id()), 0);
			Balances::mint_into(&CollatorStaking::account_id(), Balances::minimum_balance())
				.unwrap();

			assert_eq!(TotalBlocks::<Test>::get(), (1, 1));
			assert_eq!(CurrentSession::<Test>::get(), 0);
			for block in 1..=9 {
				initialize_to_block(block);
				assert_eq!(CurrentSession::<Test>::get(), 0);
				assert_eq!(TotalBlocks::<Test>::get(), (block as u32, block as u32));

				// Assume we collected one unit in fees per block
				assert_ok!(Balances::transfer(&1, &CollatorStaking::account_id(), 1, Preserve));
			}
			assert_eq!(
				Balances::balance(&CollatorStaking::account_id()),
				Balances::minimum_balance() + 9
			);
			assert!(!System::events().iter().any(|e| {
				matches!(
					e.event,
					RuntimeEvent::CollatorStaking(Event::StakingRewardReceived { .. })
				)
			}));

			assert_eq!(ProducedBlocks::<Test>::get(4), 9);
			initialize_to_block(10);
			assert_eq!(CurrentSession::<Test>::get(), 1);
			assert_eq!(TotalBlocks::<Test>::get(), (1, 1));

			// Block 10 was produced.
			assert_eq!(ProducedBlocks::<Test>::get(4), 1);

			System::assert_has_event(RuntimeEvent::CollatorStaking(Event::StakingRewardReceived {
				account: 4,
				amount: 1,
			}));

			System::reset_events();

			for block in 10..=19 {
				initialize_to_block(block);
				assert_eq!(CurrentSession::<Test>::get(), 1);
				assert_eq!(TotalBlocks::<Test>::get(), (block as u32 - 9, block as u32 - 9));

				// Assume we collected one unit in fees per block
				assert_ok!(Balances::transfer(&1, &CollatorStaking::account_id(), 1, Preserve));
			}

			assert_eq!(
				Balances::free_balance(CollatorStaking::account_id()) - Balances::minimum_balance(),
				18
			);
			initialize_to_block(20);
			System::assert_has_event(RuntimeEvent::CollatorStaking(Event::SessionEnded {
				index: 1,
				rewards: 18,
			}));
			// 18 were generated, but 3 (20%) went for collators.
			assert_eq!(
				Balances::free_balance(CollatorStaking::account_id()) - Balances::minimum_balance(),
				15
			);
			assert_eq!(CurrentSession::<Test>::get(), 2);
			assert_eq!(TotalBlocks::<Test>::get(), (1, 1));
			// This belongs to staker 4.
			assert_eq!(ClaimableRewards::<Test>::get(), 15);

			// Block 20 was produced.
			assert_eq!(ProducedBlocks::<Test>::get(4), 1);

			// Total rewards: 10 (from session 1) + 8 (from session 0) = 18
			// 3 (20%) for collators
			// 13 (80%) for stakers

			// Reward for collator
			System::assert_has_event(RuntimeEvent::CollatorStaking(Event::StakingRewardReceived {
				account: 4,
				amount: 3,
			}));
			// Reward for staker.
			assert_eq!(CollatorStaking::calculate_unclaimed_rewards(&4), 15);
			assert_ok!(CollatorStaking::claim_rewards(RuntimeOrigin::signed(4)));
			assert_eq!(ClaimableRewards::<Test>::get(), 0);
			System::assert_has_event(RuntimeEvent::CollatorStaking(Event::StakingRewardReceived {
				account: 4,
				amount: 15,
			}));
		});
	}

	#[test]
	fn should_reward_collator_with_extra_rewards_and_many_stakers() {
		new_test_ext().execute_with(|| {
			initialize_to_block(1);

			assert_ok!(CollatorStaking::register_as_candidate(
				RuntimeOrigin::signed(3),
				MinCandidacyBond::<Test>::get()
			));
			// only the candidate 4 is going to produce blocks, but we do not want the candidate 3 to be kicked.
			LastAuthoredBlock::<Test>::insert(3, 100);
			assert_ok!(CollatorStaking::register_as_candidate(
				RuntimeOrigin::signed(4),
				MinCandidacyBond::<Test>::get()
			));
			lock_for_staking(2..=5);
			assert_eq!(CollatorStaking::get_staked_balance(&3), 90);
			assert_ok!(CollatorStaking::stake(
				RuntimeOrigin::signed(2),
				vec![StakeTarget { candidate: 4, stake: 40 }].try_into().unwrap()
			));
			assert_ok!(CollatorStaking::stake(
				RuntimeOrigin::signed(3),
				vec![StakeTarget { candidate: 4, stake: 50 }].try_into().unwrap()
			));
			assert_ok!(CollatorStaking::stake(
				RuntimeOrigin::signed(5),
				vec![StakeTarget { candidate: 3, stake: 91 }].try_into().unwrap()
			));
			assert_eq!(
				candidate_list(),
				vec![
					(4, CandidateInfo { stake: 90, stakers: 2 }),
					(3, CandidateInfo { stake: 91, stakers: 1 }),
				]
			);

			// Staker 3 will autocompound 40% of its earnings
			AutoCompound::<Test>::insert(3, Percent::from_parts(40));
			ExtraReward::<Test>::set(1);
			assert_eq!(Balances::balance(&CollatorStaking::account_id()), 0);
			Balances::mint_into(&CollatorStaking::account_id(), Balances::minimum_balance())
				.unwrap();
			fund_account(CollatorStaking::extra_reward_account_id());

			assert_eq!(TotalBlocks::<Test>::get(), (1, 1));
			assert_eq!(CurrentSession::<Test>::get(), 0);
			for block in 1..=9 {
				initialize_to_block(block);
				assert_eq!(CurrentSession::<Test>::get(), 0);
				assert_eq!(TotalBlocks::<Test>::get(), (block as u32, block as u32));

				// Assume we collected one unit in fees per block
				assert_ok!(Balances::transfer(&1, &CollatorStaking::account_id(), 1, Preserve));
			}
			assert_eq!(
				Balances::balance(&CollatorStaking::account_id()),
				Balances::minimum_balance() + 9
			);
			assert!(!System::events().iter().any(|e| {
				matches!(
					e.event,
					RuntimeEvent::CollatorStaking(Event::StakingRewardReceived { .. })
				)
			}));

			assert_eq!(ProducedBlocks::<Test>::get(4), 9);
			initialize_to_block(10);
			assert_eq!(CurrentSession::<Test>::get(), 1);
			assert_eq!(TotalBlocks::<Test>::get(), (1, 1));
			// Block 10 was produced.
			assert_eq!(ProducedBlocks::<Test>::get(4), 1);

			// Reward for collator
			System::assert_has_event(RuntimeEvent::CollatorStaking(Event::StakingRewardReceived {
				account: 4,
				amount: 3,
			}));

			System::reset_events();

			for block in 10..=19 {
				initialize_to_block(block);
				assert_eq!(CurrentSession::<Test>::get(), 1);
				assert_eq!(TotalBlocks::<Test>::get(), (block as u32 - 9, block as u32 - 9));

				// Assume we collected one unit in fees per block
				assert_ok!(Balances::transfer(&1, &CollatorStaking::account_id(), 1, Preserve));
			}

			assert_eq!(
				Balances::free_balance(CollatorStaking::account_id()) - Balances::minimum_balance(),
				25
			);
			initialize_to_block(20);
			System::assert_has_event(RuntimeEvent::CollatorStaking(Event::SessionEnded {
				index: 1,
				rewards: 35,
			}));
			// 35 was distributed for rewards, but 7 (20%) went to collators.
			assert_eq!(
				Balances::free_balance(CollatorStaking::account_id()) - Balances::minimum_balance(),
				28
			);
			assert_eq!(CurrentSession::<Test>::get(), 2);
			assert_eq!(TotalBlocks::<Test>::get(), (1, 1));
			// Block 20 was produced in the next session.
			assert_eq!(ProducedBlocks::<Test>::get(4), 1);

			// Total rewards: 25 (accumulated) + 10 (extra rewards) = 35
			// 7 (20%) for collators
			//  - Staker 4: 7
			// 28 (80%) for stakers
			//  - Staker 2 -> 44.4% = 12
			//  - Staker 3 -> 55.5% = 15
			//  - rounding -> 1

			// Reward for collator
			System::assert_has_event(RuntimeEvent::CollatorStaking(Event::StakingRewardReceived {
				account: 4,
				amount: 7,
			}));
			assert_eq!(CollatorStaking::calculate_unclaimed_rewards(&2), 12);
			assert_ok!(CollatorStaking::claim_rewards(RuntimeOrigin::signed(2)));
			assert_eq!(ClaimableRewards::<Test>::get(), 16); // this remains to staker 3.
			System::assert_has_event(RuntimeEvent::CollatorStaking(Event::StakingRewardReceived {
				account: 2,
				amount: 12,
			}));
			assert_eq!(CollatorStaking::calculate_unclaimed_rewards(&3), 15);
			assert_ok!(CollatorStaking::claim_rewards(RuntimeOrigin::signed(3)));
			assert_eq!(ClaimableRewards::<Test>::get(), 1); // rounding issue
			System::assert_has_event(RuntimeEvent::CollatorStaking(Event::StakingRewardReceived {
				account: 3,
				amount: 15,
			}));

			// Check that staker 3 added 40% of its earnings via autocompound.
			System::assert_has_event(RuntimeEvent::CollatorStaking(Event::StakeAdded {
				account: 3,
				candidate: 4,
				amount: 6,
			}));
			// Staker 3 autocompounded 6 in the previous round.
			assert_eq!(CollatorStaking::get_staked_balance(&3), 96);

			// Check after adding the stake via autocompound the candidate list is sorted.
			assert_eq!(
				candidate_list(),
				vec![
					(3, CandidateInfo { stake: 91, stakers: 1 }),
					(4, CandidateInfo { stake: 96, stakers: 2 }),
				]
			);
		});
	}

	#[test]
	fn stop_extra_reward() {
		new_test_ext().execute_with(|| {
			initialize_to_block(1);

			fund_account(CollatorStaking::extra_reward_account_id());
			assert_eq!(ExtraReward::<Test>::get(), 0);

			// Cannot stop if already zero
			assert_noop!(
				CollatorStaking::stop_extra_reward(RuntimeOrigin::signed(RootAccount::get())),
				Error::<Test>::ExtraRewardAlreadyDisabled
			);

			// Now we can stop it
			assert_ok!(CollatorStaking::set_extra_reward(
				RuntimeOrigin::signed(RootAccount::get()),
				2
			));
			assert_ok!(CollatorStaking::stop_extra_reward(RuntimeOrigin::signed(
				RootAccount::get()
			)));

			System::assert_last_event(RuntimeEvent::CollatorStaking(Event::ExtraRewardRemoved {
				amount_left: 100,
				receiver: Some(40),
			}));
			assert_eq!(ExtraReward::<Test>::get(), 0);
		});
	}
}

mod session_management {
	use super::*;

	#[test]
	fn session_management_single_candidate() {
		new_test_ext().execute_with(|| {
			initialize_to_block(1);

			assert_eq!(SessionChangeBlock::get(), 0);
			assert_eq!(SessionHandlerCollators::get(), vec![1, 2]);

			initialize_to_block(4);

			assert_eq!(SessionChangeBlock::get(), 0);
			assert_eq!(SessionHandlerCollators::get(), vec![1, 2]);

			// add a new collator
			register_candidates(3..=3);

			// session won't see this.
			assert_eq!(SessionHandlerCollators::get(), vec![1, 2]);
			// but we have a new candidate.
			assert_eq!(Candidates::<Test>::count(), 1);

			initialize_to_block(10);
			assert_eq!(SessionChangeBlock::get(), 10);
			// pallet-session has 1 session delay; current validators are the same.
			assert_eq!(Session::validators(), vec![1, 2]);
			// queued ones are changed, and now we have 3.
			assert_eq!(Session::queued_keys().len(), 3);
			// session handlers (aura, et. al.) cannot see this yet.
			assert_eq!(SessionHandlerCollators::get(), vec![1, 2]);

			initialize_to_block(20);
			assert_eq!(SessionChangeBlock::get(), 20);
			// changed are now reflected to session handlers.
			assert_eq!(SessionHandlerCollators::get(), vec![1, 2, 3]);
		});
	}

	#[test]
	fn session_management_max_candidates() {
		new_test_ext().execute_with(|| {
			initialize_to_block(1);

			assert_eq!(SessionChangeBlock::get(), 0);
			assert_eq!(SessionHandlerCollators::get(), vec![1, 2]);

			initialize_to_block(4);

			assert_eq!(SessionChangeBlock::get(), 0);
			assert_eq!(SessionHandlerCollators::get(), vec![1, 2]);

			register_candidates(3..=5);
			lock_for_staking(3..=4);
			assert_ok!(CollatorStaking::stake(
				RuntimeOrigin::signed(4),
				vec![StakeTarget { candidate: 4, stake: 50 }].try_into().unwrap()
			));
			assert_ok!(CollatorStaking::stake(
				RuntimeOrigin::signed(3),
				vec![StakeTarget { candidate: 3, stake: 60 }].try_into().unwrap()
			));

			// session won't see this.
			assert_eq!(SessionHandlerCollators::get(), vec![1, 2]);
			// but we have a new candidate.
			assert_eq!(Candidates::<Test>::count(), 3);

			initialize_to_block(10);
			assert_eq!(SessionChangeBlock::get(), 10);
			// pallet-session has 1 session delay; current validators are the same.
			assert_eq!(Session::validators(), vec![1, 2]);
			// queued ones are changed, and now we have 4.
			assert_eq!(Session::queued_keys().len(), 4);
			// session handlers (aura, et. al.) cannot see this yet.
			assert_eq!(SessionHandlerCollators::get(), vec![1, 2]);

			initialize_to_block(20);
			assert_eq!(SessionChangeBlock::get(), 20);
			// changes are now reflected to session handlers.
			assert_eq!(SessionHandlerCollators::get(), vec![1, 2, 3, 4]);
		});
	}

	#[test]
	fn session_management_increase_bid_with_list_update() {
		new_test_ext().execute_with(|| {
			initialize_to_block(1);

			assert_eq!(SessionChangeBlock::get(), 0);
			assert_eq!(SessionHandlerCollators::get(), vec![1, 2]);

			initialize_to_block(4);

			assert_eq!(SessionChangeBlock::get(), 0);
			assert_eq!(SessionHandlerCollators::get(), vec![1, 2]);

			register_candidates(3..=5);
			lock_for_staking(3..=5);
			assert_ok!(CollatorStaking::stake(
				RuntimeOrigin::signed(5),
				vec![StakeTarget { candidate: 5, stake: 60 }].try_into().unwrap()
			));
			assert_ok!(CollatorStaking::stake(
				RuntimeOrigin::signed(3),
				vec![StakeTarget { candidate: 3, stake: 50 }].try_into().unwrap()
			));

			// session won't see this.
			assert_eq!(SessionHandlerCollators::get(), vec![1, 2]);
			// but we have a new candidate.
			assert_eq!(Candidates::<Test>::count(), 3);

			initialize_to_block(10);
			assert_eq!(SessionChangeBlock::get(), 10);
			// pallet-session has 1 session delay; current validators are the same.
			assert_eq!(Session::validators(), vec![1, 2]);
			// queued ones are changed, and now we have 4.
			assert_eq!(Session::queued_keys().len(), 4);
			// session handlers (aura, et. al.) cannot see this yet.
			assert_eq!(SessionHandlerCollators::get(), vec![1, 2]);

			initialize_to_block(20);
			assert_eq!(SessionChangeBlock::get(), 20);
			// changed are now reflected to session handlers.
			assert_eq!(SessionHandlerCollators::get(), vec![1, 2, 5, 3]);
		});
	}

	#[test]
	fn session_management_candidate_list_eager_sort() {
		new_test_ext().execute_with(|| {
			initialize_to_block(1);

			assert_eq!(SessionChangeBlock::get(), 0);
			assert_eq!(SessionHandlerCollators::get(), vec![1, 2]);

			initialize_to_block(4);

			assert_eq!(SessionChangeBlock::get(), 0);
			assert_eq!(SessionHandlerCollators::get(), vec![1, 2]);

			register_candidates(3..=5);
			lock_for_staking(5..=5);
			assert_ok!(CollatorStaking::stake(
				RuntimeOrigin::signed(5),
				vec![StakeTarget { candidate: 5, stake: 60 }].try_into().unwrap()
			));

			// session won't see this.
			assert_eq!(SessionHandlerCollators::get(), vec![1, 2]);
			// but we have a new candidate.
			assert_eq!(Candidates::<Test>::count(), 3);

			initialize_to_block(10);
			assert_eq!(SessionChangeBlock::get(), 10);
			// pallet-session has 1 session delay; current validators are the same.
			assert_eq!(Session::validators(), vec![1, 2]);
			// queued ones are changed, and now we have 4.
			assert_eq!(Session::queued_keys().len(), 4);
			// session handlers (aura, et. al.) cannot see this yet.
			assert_eq!(SessionHandlerCollators::get(), vec![1, 2]);

			initialize_to_block(20);
			assert_eq!(SessionChangeBlock::get(), 20);
			// changed are now reflected to session handlers.
			assert_eq!(SessionHandlerCollators::get(), vec![1, 2, 5, 3]);
		});
	}

	#[test]
	fn session_management_reciprocal_outbidding() {
		new_test_ext().execute_with(|| {
			initialize_to_block(1);

			assert_eq!(SessionChangeBlock::get(), 0);
			assert_eq!(SessionHandlerCollators::get(), vec![1, 2]);

			initialize_to_block(4);

			assert_eq!(SessionChangeBlock::get(), 0);
			assert_eq!(SessionHandlerCollators::get(), vec![1, 2]);

			register_candidates(3..=5);
			lock_for_staking(3..=5);

			assert_ok!(CollatorStaking::stake(
				RuntimeOrigin::signed(5),
				vec![StakeTarget { candidate: 5, stake: 60 }].try_into().unwrap()
			));

			initialize_to_block(5);

			// candidates 3 and 4 saw they were outbid and preemptively bid more
			// than 5 in the next block.
			assert_ok!(CollatorStaking::stake(
				RuntimeOrigin::signed(4),
				vec![StakeTarget { candidate: 4, stake: 80 }].try_into().unwrap()
			));
			assert_ok!(CollatorStaking::stake(
				RuntimeOrigin::signed(3),
				vec![StakeTarget { candidate: 3, stake: 70 }].try_into().unwrap()
			));

			// session won't see this.
			assert_eq!(SessionHandlerCollators::get(), vec![1, 2]);
			// but we have a new candidate.
			assert_eq!(Candidates::<Test>::count(), 3);

			initialize_to_block(10);
			assert_eq!(SessionChangeBlock::get(), 10);
			// pallet-session has 1 session delay; current validators are the same.
			assert_eq!(Session::validators(), vec![1, 2]);
			// queued ones are changed, and now we have 4.
			assert_eq!(Session::queued_keys().len(), 4);
			// session handlers (aura, et. al.) cannot see this yet.
			assert_eq!(SessionHandlerCollators::get(), vec![1, 2]);

			initialize_to_block(20);
			assert_eq!(SessionChangeBlock::get(), 20);
			// changed are now reflected to session handlers.
			assert_eq!(SessionHandlerCollators::get(), vec![1, 2, 4, 3]);
		});
	}

	#[test]
	fn session_management_decrease_bid_after_auction() {
		new_test_ext().execute_with(|| {
			initialize_to_block(1);

			assert_eq!(SessionChangeBlock::get(), 0);
			assert_eq!(SessionHandlerCollators::get(), vec![1, 2]);

			initialize_to_block(4);

			assert_eq!(SessionChangeBlock::get(), 0);
			assert_eq!(SessionHandlerCollators::get(), vec![1, 2]);

			register_candidates(3..=5);
			lock_for_staking(3..=5);

			assert_ok!(CollatorStaking::stake(
				RuntimeOrigin::signed(5),
				vec![StakeTarget { candidate: 5, stake: 60 }].try_into().unwrap()
			));

			initialize_to_block(5);

			assert_ok!(CollatorStaking::stake(
				RuntimeOrigin::signed(4),
				vec![StakeTarget { candidate: 4, stake: 80 }].try_into().unwrap()
			));
			assert_ok!(CollatorStaking::stake(
				RuntimeOrigin::signed(3),
				vec![StakeTarget { candidate: 3, stake: 70 }].try_into().unwrap()
			));

			initialize_to_block(5);

			// candidate 5 saw it was outbid and wants to take back its bid, but
			// not entirely so, they still keep their place in the candidate list
			// in case there is an opportunity in the future.
			assert_ok!(CollatorStaking::unstake_from(RuntimeOrigin::signed(5), 5));

			// session won't see this.
			assert_eq!(SessionHandlerCollators::get(), vec![1, 2]);
			// but we have a new candidate.
			assert_eq!(Candidates::<Test>::count(), 3);

			initialize_to_block(10);
			assert_eq!(SessionChangeBlock::get(), 10);
			// pallet-session has 1 session delay; current validators are the same.
			assert_eq!(Session::validators(), vec![1, 2]);
			// queued ones are changed, and now we have 4.
			assert_eq!(Session::queued_keys().len(), 4);
			// session handlers (aura, et. al.) cannot see this yet.
			assert_eq!(SessionHandlerCollators::get(), vec![1, 2]);

			initialize_to_block(20);
			assert_eq!(SessionChangeBlock::get(), 20);
			// changes are now reflected to session handlers.
			assert_eq!(SessionHandlerCollators::get(), vec![1, 2, 4, 3]);
		});
	}

	#[test]
	fn kick_mechanism() {
		new_test_ext().execute_with(|| {
			initialize_to_block(1);

			// add a new collator
			assert_ok!(CollatorStaking::register_as_candidate(
				RuntimeOrigin::signed(3),
				MinCandidacyBond::<Test>::get()
			));
			assert_ok!(CollatorStaking::register_as_candidate(
				RuntimeOrigin::signed(4),
				MinCandidacyBond::<Test>::get()
			));
			initialize_to_block(10);
			assert_eq!(Candidates::<Test>::count(), 2);
			initialize_to_block(30);
			assert_eq!(SessionChangeBlock::get(), 30);
			// 4 authored this block, gets to stay 3 was kicked
			assert_eq!(Candidates::<Test>::count(), 1);
			// 3 will be kicked after 1 session delay
			assert_eq!(SessionHandlerCollators::get(), vec![1, 2, 3, 4]);
			assert_eq!(candidate_list(), vec![(4, CandidateInfo { stake: 0, stakers: 0 })]);
			assert_eq!(LastAuthoredBlock::<Test>::get(4), 30);
			initialize_to_block(40);
			// 3 gets kicked after 1 session delay
			assert_eq!(SessionHandlerCollators::get(), vec![1, 2, 4]);
			// kicked collator gets funds back after a delay
			assert_eq!(Balances::balance_frozen(&FreezeReason::CandidacyBond.into(), &3), 0);
			assert_eq!(Balances::balance_frozen(&FreezeReason::Releasing.into(), &3), 10);
			assert_eq!(
				CandidacyBondReleases::<Test>::get(3),
				Some(CandidacyBondRelease {
					bond: 10,
					block: 35,
					reason: CandidacyBondReleaseReason::Idle
				})
			);
		});
	}

	#[test]
	fn should_not_kick_mechanism_too_few() {
		new_test_ext().execute_with(|| {
			initialize_to_block(1);

			// remove the invulnerables and add new collators 3 and 5
			assert_eq!(Candidates::<Test>::count(), 0);
			assert_eq!(Invulnerables::<Test>::get(), vec![1, 2]);
			assert_ok!(CollatorStaking::remove_invulnerable(
				RuntimeOrigin::signed(RootAccount::get()),
				1
			));
			assert_ok!(CollatorStaking::register_as_candidate(
				RuntimeOrigin::signed(3),
				MinCandidacyBond::<Test>::get()
			));
			assert_ok!(CollatorStaking::register_as_candidate(
				RuntimeOrigin::signed(5),
				MinCandidacyBond::<Test>::get()
			));
			assert_ok!(CollatorStaking::remove_invulnerable(
				RuntimeOrigin::signed(RootAccount::get()),
				2
			));

			initialize_to_block(20);
			assert_eq!(Candidates::<Test>::count(), 2);

			initialize_to_block(30);
			assert_eq!(SessionChangeBlock::get(), 30);
			// 4 authored this block, 3 is kicked, 5 stays because of too few collators
			assert_eq!(Candidates::<Test>::count(), 1);
			// 3 will be kicked after 1 session delay
			assert_eq!(SessionHandlerCollators::get(), vec![5, 3]);
			// tuple of (id, deposit).
			let collator = CandidateInfo { stake: 0, stakers: 0 };
			assert_eq!(candidate_list(), vec![(3, collator)]);
			assert_eq!(LastAuthoredBlock::<Test>::get(4), 30);

			initialize_to_block(40);
			// 3 gets kicked after 1 session delay
			assert_eq!(SessionHandlerCollators::get(), vec![3]);
			// kicked collator gets funds back after a delay
			assert_eq!(Balances::balance_frozen(&FreezeReason::Releasing.into(), &5), 10);
			assert_eq!(
				CandidacyBondReleases::<Test>::get(5),
				Some(CandidacyBondRelease {
					bond: 10,
					block: 35,
					reason: CandidacyBondReleaseReason::Idle
				})
			);
		});
	}
}

mod claim_rewards_other {
	use super::*;

	#[test]
	fn claim_rewards_other_should_work() {
		new_test_ext().execute_with(|| {
			initialize_to_block(1);

			// Register a candidate
			register_candidates(3..=3);
			lock_for_staking(4..=4);

			// Staker 4 stakes on candidate 3
			assert_ok!(CollatorStaking::stake(
				RuntimeOrigin::signed(4),
				vec![StakeTarget { candidate: 3, stake: 40 }].try_into().unwrap()
			));

			// Move 1 session.
			initialize_to_block(10);

			// Simulate that collator 3 produced blocks in session 1
			ProducedBlocks::<Test>::insert(3, 10);

			// Generate 10 as rewards on the pot generated during session 1.
			// 20% to collators, 80% to stakers, so 8 to staker.
			assert_ok!(Balances::mint_into(
				&CollatorStaking::account_id(),
				Balances::minimum_balance() + 10
			));

			// Move to session 2.
			initialize_to_block(20);
			System::assert_has_event(RuntimeEvent::CollatorStaking(Event::SessionEnded {
				index: 1,
				rewards: 10,
			}));

			// Check that staker 4 has 8 claimable rewards.
			assert_eq!(CollatorStaking::calculate_unclaimed_rewards(&4), 8);

			// Claim rewards for staker 4 from other user.
			assert_ok!(CollatorStaking::claim_rewards_other(RuntimeOrigin::signed(1), 4));
			System::assert_has_event(RuntimeEvent::CollatorStaking(Event::StakingRewardReceived {
				account: 4,
				amount: 8, // 80% of 10
			}));
		});
	}

	#[test]
	fn nonexistent_account() {
		new_test_ext().execute_with(|| {
			initialize_to_block(1);

			// Try to claim rewards for an account that has never staked
			let nonexistent_staker = 25;

			assert_noop!(
				CollatorStaking::claim_rewards_other(RuntimeOrigin::signed(1), nonexistent_staker),
				Error::<Test>::NoPendingClaim
			);
		});
	}

	#[test]
	fn requires_signed_origin() {
		new_test_ext().execute_with(|| {
			initialize_to_block(1);

			// Try to claim with root origin should fail
			assert_noop!(CollatorStaking::claim_rewards_other(RuntimeOrigin::root(), 4), BadOrigin);
		});
	}
}
