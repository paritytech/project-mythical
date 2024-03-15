use crate::{mock::*, *};
use frame_support::{
	assert_noop, assert_ok, error::BadOrigin
};

type AccountIdOf<Test> = <Test as frame_system::Config>::AccountId;

fn account(id: u8) -> AccountIdOf<Test> {
	[id; 32].into()
}

mod force_set_authority {
	use super::*;
	// Force set Authority
	#[test]
	fn force_set_authoity_works() {
		new_test_ext().execute_with(|| {
			assert_ok!(Marketplace::force_set_authority(RuntimeOrigin::root(), account(1)));
			assert!(Marketplace::authority() == Some(account(1)));
		})
	}

	#[test]
	fn fails_no_root() {
		new_test_ext().execute_with(|| {
			assert_noop!(
				Marketplace::force_set_authority(RuntimeOrigin::signed(account(1)), account(1)),
				BadOrigin
			);
		})
	}

	#[test]
	fn fails_account_already_set() {
		new_test_ext().execute_with(|| {
			assert_ok!(Marketplace::force_set_authority(RuntimeOrigin::root(), account(1)));

			assert_noop!(
				Marketplace::force_set_authority(RuntimeOrigin::root(), account(1)),
				Error::<Test>::AccountAlreadySet
			);
		})
	}
}

// Set Fee Signer
mod set_fee_signer {
	use super::*;
	// Force set Authority
	#[test]
	fn set_fee_signer_works() {
		new_test_ext().execute_with(|| {
			//Add Marketplace authority
			assert_ok!(Marketplace::force_set_authority(RuntimeOrigin::root(), account(1)));

			assert_ok!(Marketplace::set_fee_signer_address(
				RuntimeOrigin::signed(account(1)),
				account(2)
			));
			assert!(Marketplace::fee_signer() == Some(account(2)));
		})
	}

	#[test]
	fn fails_not_authority() {
		new_test_ext().execute_with(|| {
			//No authority set
			assert_noop!(
				Marketplace::set_fee_signer_address(RuntimeOrigin::signed(account(1)), account(1)),
				Error::<Test>::NotAuthority
			);

			//Add Marketplace authority
			assert_ok!(Marketplace::force_set_authority(RuntimeOrigin::root(), account(1)));

			// Fails if wrong authority
			assert_noop!(
				Marketplace::set_fee_signer_address(RuntimeOrigin::signed(account(2)), account(1)),
				Error::<Test>::NotAuthority
			);
		})
	}

	#[test]
	fn fails_account_already_set() {
		new_test_ext().execute_with(|| {
			assert_ok!(Marketplace::force_set_authority(RuntimeOrigin::root(), account(1)));
			assert_ok!(Marketplace::set_fee_signer_address(
				RuntimeOrigin::signed(account(1)),
				account(2)
			));

			assert_noop!(
				Marketplace::set_fee_signer_address(RuntimeOrigin::signed(account(1)), account(2)),
				Error::<Test>::AccountAlreadySet
			);
		})
	}
}
// Set Payout Address
mod set_payout_address {
	use super::*;
	// Force set Authority
	#[test]
	fn set_payout_address_works() {
		new_test_ext().execute_with(|| {
			//Add Marketplace authority
			assert_ok!(Marketplace::force_set_authority(RuntimeOrigin::root(), account(1)));

			assert_ok!(Marketplace::set_payout_address(
				RuntimeOrigin::signed(account(1)),
				account(2)
			));
			assert!(Marketplace::payout_address() == Some(account(2)));
		})
	}

	#[test]
	fn fails_not_authority() {
		new_test_ext().execute_with(|| {
			//No authority set
			assert_noop!(
				Marketplace::set_payout_address(RuntimeOrigin::signed(account(1)), account(1)),
				Error::<Test>::NotAuthority
			);

			//Add Marketplace authority
			assert_ok!(Marketplace::force_set_authority(RuntimeOrigin::root(), account(1)));

			// Fails if wrong authority
			assert_noop!(
				Marketplace::set_payout_address(RuntimeOrigin::signed(account(2)), account(1)),
				Error::<Test>::NotAuthority
			);
		})
	}

	#[test]
	fn fails_account_already_set() {
		new_test_ext().execute_with(|| {
			assert_ok!(Marketplace::force_set_authority(RuntimeOrigin::root(), account(1)));
			assert_ok!(Marketplace::set_payout_address(
				RuntimeOrigin::signed(account(1)),
				account(2)
			));

			assert_noop!(
				Marketplace::set_payout_address(RuntimeOrigin::signed(account(1)), account(2)),
				Error::<Test>::AccountAlreadySet
			);
		})
	}
}
