use crate::{mock::*, *};
use frame_support::{assert_noop, assert_ok, error::BadOrigin};

type AccountIdOf<Test> = <Test as frame_system::Config>::AccountId;

fn account(id: u8) -> AccountIdOf<Test> {
	[id; 32].into()
}

mod force_set_migrator {
	use super::*;
	// Force set Authority
	#[test]
	fn force_set_migrator_works() {
		new_test_ext().execute_with(|| {
			assert_ok!(Migration::force_set_migrator(RuntimeOrigin::root(), account(1)));
			assert!(Migration::migrator() == Some(account(1)));
		})
	}

	#[test]
	fn fails_no_root() {
		new_test_ext().execute_with(|| {
			assert_noop!(
				Migration::force_set_migrator(RuntimeOrigin::signed(account(1)), account(1)),
				BadOrigin
			);
		})
	}

	#[test]
	fn fails_account_already_set() {
		new_test_ext().execute_with(|| {
			assert_ok!(Migration::force_set_migrator(RuntimeOrigin::root(), account(1)));

			assert_noop!(
				Migration::force_set_migrator(RuntimeOrigin::root(), account(1)),
				Error::<Test>::AccountAlreadySet
			);
		})
	}
}
