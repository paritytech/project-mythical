use crate::{mock::*, *};
use frame_support::{assert_noop, assert_ok, error::BadOrigin, traits::fungible::Mutate};
use frame_system::pallet_prelude::BlockNumberFor;
use pallet_marketplace::{Ask, Asks};
use pallet_nfts::{CollectionConfig, CollectionSettings, MintSettings};

type AccountIdOf<Test> = <Test as frame_system::Config>::AccountId;
type Balance<Test> = <Test as pallet_balances::Config>::Balance;
type CollectionId<Test> = <Test as pallet_nfts::Config>::CollectionId;

fn account(id: u8) -> AccountIdOf<Test> {
	[id; 32].into()
}

fn mint_item(item: u32, owner: AccountIdOf<Test>) {
	Balances::set_balance(&account(1), 100000);
	if Nfts::collection_owner(0) == None {
		assert_ok!(Nfts::create(
			RuntimeOrigin::signed(account(1)),
			account(1),
			collection_config_with_all_settings_enabled()
		));
	};
	assert_ok!(Nfts::mint(RuntimeOrigin::signed(account(1)), 0, item, owner, None));
}

fn collection_config_with_all_settings_enabled(
) -> CollectionConfig<Balance<Test>, BlockNumberFor<Test>, CollectionId<Test>> {
	CollectionConfig {
		settings: CollectionSettings::all_enabled(),
		max_supply: None,
		mint_settings: MintSettings::default(),
	}
}

mod force_set_migrator {
	use super::*;

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
}

mod set_next_collection_id {
	use super::*;

	#[test]
	fn set_next_collection_id_works() {
		new_test_ext().execute_with(|| {
			assert_ok!(Migration::force_set_migrator(RuntimeOrigin::root(), account(1)));
			assert_ok!(Migration::set_next_collection_id(RuntimeOrigin::signed(account(1)), 25));
			assert!(Migration::get_next_id() == 25);
		})
	}
	#[test]
	fn fails_no_migrator() {
		new_test_ext().execute_with(|| {
			assert_noop!(
				Migration::set_next_collection_id(RuntimeOrigin::signed(account(1)), 25),
				Error::<Test>::MigratorNotSet
			);
			assert_ok!(Migration::force_set_migrator(RuntimeOrigin::root(), account(1)));
			assert_noop!(
				Migration::set_next_collection_id(RuntimeOrigin::signed(account(2)), 25),
				Error::<Test>::NotMigrator
			);
		})
	}
}

mod create_ask {
	use super::*;

	#[test]
	fn creator_is_not_migrator_fails() {
		new_test_ext().execute_with(|| {
			assert_ok!(Migration::force_set_migrator(RuntimeOrigin::root(), account(1)));
			assert_noop!(
				Migration::create_ask(
					RuntimeOrigin::signed(account(2)),
					0,
					0,
					Ask { seller: account(1), price: 10000, expiration: 10000, fee: 1 }
				),
				Error::<Test>::NotMigrator
			);
		})
	}

	#[test]
	fn item_not_found_fails() {
		new_test_ext().execute_with(|| {
			assert_ok!(Migration::force_set_migrator(RuntimeOrigin::root(), account(1)));
			assert_noop!(
				Migration::create_ask(
					RuntimeOrigin::signed(account(1)),
					0,
					0,
					Ask { seller: account(1), price: 10000, expiration: 10000, fee: 1 }
				),
				Error::<Test>::ItemNotFound
			);
		})
	}

	#[test]
	fn invalid_expiration_fails() {
		new_test_ext().execute_with(|| {
			assert_ok!(Migration::force_set_migrator(RuntimeOrigin::root(), account(1)));
			mint_item(0, account(1));
			assert_noop!(
				Migration::create_ask(
					RuntimeOrigin::signed(account(1)),
					0,
					0,
					Ask { seller: account(1), price: 10000, expiration: 0, fee: 1 }
				),
				Error::<Test>::InvalidExpiration
			);
		})
	}

	#[test]
	fn create_ask_passes() {
		new_test_ext().execute_with(|| {
			assert_ok!(Migration::force_set_migrator(RuntimeOrigin::root(), account(1)));
			mint_item(0, account(1));
			let ask = Ask { seller: account(1), price: 10000, expiration: 10000, fee: 1 };
			assert_ok!(Migration::create_ask(RuntimeOrigin::signed(account(1)), 0, 0, ask.clone()));
			assert!(Asks::<Test>::get(0, 0) == Some(ask));
		})
	}

	#[test]
	fn create_ask_on_disabled_transfer_fails() {
		new_test_ext().execute_with(|| {
			assert_ok!(Migration::force_set_migrator(RuntimeOrigin::root(), account(1)));
			mint_item(0, account(1));
			let ask = Ask { seller: account(1), price: 10000, expiration: 10000, fee: 1 };
			assert_ok!(Migration::create_ask(RuntimeOrigin::signed(account(1)), 0, 0, ask.clone()));
			assert_noop!(
				Migration::create_ask(RuntimeOrigin::signed(account(1)), 0, 0, ask.clone()),
				pallet_nfts::Error::<Test>::ItemLocked
			);
		})
	}
}

mod set_pot_account {
	use super::*;

	#[test]
	fn set_pot_account_works() {
		new_test_ext().execute_with(|| {
			assert_ok!(Migration::force_set_migrator(RuntimeOrigin::root(), account(1)));
			assert_ok!(Migration::set_pot_account(RuntimeOrigin::signed(account(1)), account(1)));
			assert!(Migration::pot() == Some(account(1)));
		})
	}

	#[test]
	fn fails_no_migrator() {
		new_test_ext().execute_with(|| {
			assert_noop!(
				Migration::set_pot_account(RuntimeOrigin::signed(account(1)), account(1)),
				Error::<Test>::MigratorNotSet
			);
			assert_ok!(Migration::force_set_migrator(RuntimeOrigin::root(), account(1)));
			assert_noop!(
				Migration::set_pot_account(RuntimeOrigin::signed(account(2)), account(1)),
				Error::<Test>::NotMigrator
			);
		})
	}
}
mod send_funds_from_pot {
	use super::*;

	#[test]
	fn sender_is_not_migrator_fails() {
		new_test_ext().execute_with(|| {
			assert_ok!(Migration::force_set_migrator(RuntimeOrigin::root(), account(1)));
			assert_noop!(
				Migration::send_funds_from_pot(
					RuntimeOrigin::signed(account(2)),
					account(2),
					10000
				),
				Error::<Test>::NotMigrator
			);
		})
	}
	#[test]
	fn pot_not_set_fails() {
		new_test_ext().execute_with(|| {
			assert_ok!(Migration::force_set_migrator(RuntimeOrigin::root(), account(1)));
			assert_noop!(
				Migration::send_funds_from_pot(
					RuntimeOrigin::signed(account(1)),
					account(2),
					10000
				),
				Error::<Test>::PotAccountNotSet
			);
		})
	}

	#[test]
	fn send_funds_from_pot_passes() {
		new_test_ext().execute_with(|| {
			assert_ok!(Migration::force_set_migrator(RuntimeOrigin::root(), account(1)));
			Balances::set_balance(&account(1), 100000);
			assert_ok!(Migration::set_pot_account(RuntimeOrigin::signed(account(1)), account(1)));
			assert_ok!(Migration::send_funds_from_pot(
				RuntimeOrigin::signed(account(1)),
				account(2),
				10000
			));
			assert!(Balances::free_balance(&account(1)) == 90000);
			assert!(Balances::free_balance(&account(2)) == 10000);
		})
	}
}
