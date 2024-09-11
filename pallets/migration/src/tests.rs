use crate::{mock::*, *};
use frame_support::{
	assert_noop, assert_ok, dispatch::Pays, error::BadOrigin, traits::fungible::Mutate,
};
use frame_system::pallet_prelude::BlockNumberFor;
use pallet_nfts::{CollectionConfig, CollectionSettings, ItemConfig, ItemSettings, MintSettings};

type AccountIdOf<Test> = <Test as frame_system::Config>::AccountId;
type Balance<Test> = <Test as pallet_balances::Config>::Balance;
type CollectionId<Test> = <Test as pallet_nfts::Config>::CollectionId;

fn account(id: u8) -> AccountIdOf<Test> {
	[id; 32].into()
}

fn mint_item(item: u128, owner: AccountIdOf<Test>) {
	Balances::set_balance(&account(1), 100000);
	if Nfts::collection_owner(0).is_none() {
		assert_ok!(Nfts::create(
			RuntimeOrigin::signed(account(1)),
			account(1),
			collection_config_with_all_settings_enabled()
		));
	};
	assert_ok!(Nfts::mint(RuntimeOrigin::signed(account(1)), 0, Some(item), owner, None));
}

fn collection_config_with_all_settings_enabled(
) -> CollectionConfig<Balance<Test>, BlockNumberFor<Test>, CollectionId<Test>> {
	CollectionConfig {
		settings: CollectionSettings::all_enabled(),
		max_supply: Some(u128::MAX),
		mint_settings: MintSettings::default(),
	}
}

fn default_item_config() -> ItemConfig {
	ItemConfig { settings: ItemSettings::all_enabled() }
}

mod force_set_migrator {
	use super::*;

	#[test]
	fn force_set_migrator_works() {
		new_test_ext().execute_with(|| {
			assert_ok!(Migration::force_set_migrator(RuntimeOrigin::root(), account(1)));
			assert_eq!(Migration::migrator(), Some(account(1)));
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

mod set_item_owner {
	use super::*;

	#[test]
	fn sender_is_not_migrator_fails() {
		new_test_ext().execute_with(|| {
			assert_ok!(Migration::force_set_migrator(RuntimeOrigin::root(), account(1)));
			mint_item(1, account(1));
			assert_ok!(Dmarket::force_set_collection(RuntimeOrigin::root(), 0));

			assert_noop!(
				Migration::set_item_owner(RuntimeOrigin::signed(account(2)), 0, account(2)),
				Error::<Test>::NotMigrator
			);
		})
	}

	#[test]
	fn dmarket_collection_not_set() {
		new_test_ext().execute_with(|| {
			assert_ok!(Migration::force_set_migrator(RuntimeOrigin::root(), account(1)));
			assert_noop!(
				Migration::set_item_owner(RuntimeOrigin::signed(account(1)), 0, account(2)),
				Error::<Test>::DmarketCollectionNotSet
			);
		})
	}

	#[test]
	fn item_not_found_fails() {
		new_test_ext().execute_with(|| {
			assert_ok!(Migration::force_set_migrator(RuntimeOrigin::root(), account(1)));
			mint_item(1, account(1));
			assert_ok!(Dmarket::force_set_collection(RuntimeOrigin::root(), 0));

			assert_noop!(
				Migration::set_item_owner(RuntimeOrigin::signed(account(1)), 0, account(2)),
				Error::<Test>::ItemNotFound
			);
		})
	}

	#[test]
	fn already_owner_fails() {
		new_test_ext().execute_with(|| {
			assert_ok!(Migration::force_set_migrator(RuntimeOrigin::root(), account(1)));
			mint_item(1, account(1));
			assert_ok!(Dmarket::force_set_collection(RuntimeOrigin::root(), 0));

			assert_noop!(
				Migration::set_item_owner(RuntimeOrigin::signed(account(1)), 1, account(1)),
				Error::<Test>::AlreadyOwner
			);
		})
	}

	#[test]
	fn set_item_owner_passes() {
		new_test_ext().execute_with(|| {
			assert_ok!(Migration::force_set_migrator(RuntimeOrigin::root(), account(1)));
			mint_item(1, account(1));
			assert_ok!(Dmarket::force_set_collection(RuntimeOrigin::root(), 0));

			let res = Migration::set_item_owner(RuntimeOrigin::signed(account(1)), 1, account(2));
			assert!(res.is_ok());
			assert_eq!(res.unwrap().pays_fee, Pays::No);

			assert_eq!(Nfts::owner(0, 1), Some(account(2)));
		})
	}
}

mod set_team {
	use super::*;

	#[test]
	fn sender_is_not_migrator_fails() {
		new_test_ext().execute_with(|| {
			assert_ok!(Migration::force_set_migrator(RuntimeOrigin::root(), account(1)));
			assert_noop!(
				Migration::set_team(
					RuntimeOrigin::signed(account(2)),
					Some(account(3)),
					Some(account(3)),
					Some(account(3))
				),
				Error::<Test>::NotMigrator
			);
		})
	}

	#[test]
	fn dmarket_collection_not_set() {
		new_test_ext().execute_with(|| {
			assert_ok!(Migration::force_set_migrator(RuntimeOrigin::root(), account(2)));
			assert_ok!(Nfts::force_create(
				RuntimeOrigin::signed(account(2)),
				account(3),
				collection_config_with_all_settings_enabled()
			));

			assert_noop!(
				Migration::set_team(
					RuntimeOrigin::signed(account(2)),
					Some(account(3)),
					Some(account(3)),
					Some(account(3)),
				),
				Error::<Test>::DmarketCollectionNotSet
			);
		})
	}

	#[test]
	fn set_team_redispatch_works() {
		new_test_ext().execute_with(|| {
			assert_ok!(Migration::force_set_migrator(RuntimeOrigin::root(), account(2)));
			assert_ok!(Nfts::force_create(
				RuntimeOrigin::signed(account(2)),
				account(3),
				collection_config_with_all_settings_enabled()
			));
			assert_ok!(Dmarket::force_set_collection(RuntimeOrigin::root(), 0));

			let res = Migration::set_team(
				RuntimeOrigin::signed(account(2)),
				Some(account(3)),
				Some(account(3)),
				Some(account(3)),
			);
			assert!(res.is_ok());
			assert_eq!(res.unwrap().pays_fee, Pays::No)
		})
	}
}

mod force_mint {
	use super::*;

	#[test]
	fn sender_is_not_migrator_fails() {
		new_test_ext().execute_with(|| {
			assert_ok!(Migration::force_set_migrator(RuntimeOrigin::root(), account(1)));
			assert_noop!(
				Migration::force_mint(
					RuntimeOrigin::signed(account(2)),
					0,
					account(3),
					default_item_config()
				),
				Error::<Test>::NotMigrator
			);
		})
	}

	#[test]
	fn dmarket_collection_not_set() {
		new_test_ext().execute_with(|| {
			assert_ok!(Migration::force_set_migrator(RuntimeOrigin::root(), account(2)));
			assert_ok!(Nfts::force_create(
				RuntimeOrigin::signed(account(2)),
				account(3),
				collection_config_with_all_settings_enabled()
			));

			assert_noop!(
				Migration::force_mint(
					RuntimeOrigin::signed(account(2)),
					1,
					account(3),
					default_item_config(),
				),
				Error::<Test>::DmarketCollectionNotSet
			);
		})
	}
	#[test]
	fn force_mint_redispatch_works() {
		new_test_ext().execute_with(|| {
			assert_ok!(Migration::force_set_migrator(RuntimeOrigin::root(), account(2)));
			assert_ok!(Nfts::force_create(
				RuntimeOrigin::signed(account(2)),
				account(3),
				collection_config_with_all_settings_enabled()
			));
			assert_ok!(Dmarket::force_set_collection(RuntimeOrigin::root(), 0));

			let res = Migration::force_mint(
				RuntimeOrigin::signed(account(2)),
				1,
				account(3),
				default_item_config(),
			);
			assert!(res.is_ok());
			assert_eq!(res.unwrap().pays_fee, Pays::No)
		})
	}
}
