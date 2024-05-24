use crate::{mock::*, *};
use frame_support::{
	assert_noop, assert_ok,
	dispatch::Pays,
	error::BadOrigin,
	traits::{fungible::Mutate, nonfungibles_v2::Inspect},
};
use frame_system::pallet_prelude::BlockNumberFor;
use pallet_marketplace::{Ask, Asks};
use pallet_nfts::{CollectionConfig, CollectionSettings, ItemConfig, ItemSettings, MintSettings};
use sp_runtime::ArithmeticError;

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

fn default_item_config() -> ItemConfig {
	ItemConfig { settings: ItemSettings::all_enabled() }
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

			let res = Migration::set_next_collection_id(RuntimeOrigin::signed(account(1)), 25);
			assert!(res.is_ok());
			assert!(res.unwrap().pays_fee == Pays::No);

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
					Ask {
						seller: account(1),
						price: 10000,
						expiration: 10000,
						fee: 1,
						escrow_agent: None
					}
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
					Ask {
						seller: account(1),
						price: 10000,
						expiration: 10000,
						fee: 1,
						escrow_agent: None
					}
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
					Ask {
						seller: account(1),
						price: 10000,
						expiration: 0,
						fee: 1,
						escrow_agent: None
					}
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
			let ask = Ask {
				seller: account(1),
				price: 10000,
				expiration: 10000,
				fee: 1,
				escrow_agent: None,
			};

			let res = Migration::create_ask(RuntimeOrigin::signed(account(1)), 0, 0, ask.clone());
			assert!(res.is_ok());
			assert!(res.unwrap().pays_fee == Pays::No);

			assert!(Asks::<Test>::get(0, 0) == Some(ask));
			assert!(!Nfts::can_transfer(&0, &0));
		})
	}

	#[test]
	fn create_ask_on_disabled_transfer_fails() {
		new_test_ext().execute_with(|| {
			assert_ok!(Migration::force_set_migrator(RuntimeOrigin::root(), account(1)));
			mint_item(0, account(1));
			let ask = Ask {
				seller: account(1),
				price: 10000,
				expiration: 10000,
				fee: 1,
				escrow_agent: None,
			};
			assert_ok!(Migration::create_ask(RuntimeOrigin::signed(account(1)), 0, 0, ask.clone()));
			assert_noop!(
				Migration::create_ask(RuntimeOrigin::signed(account(1)), 0, 0, ask.clone()),
				pallet_nfts::Error::<Test>::ItemLocked
			);
		})
	}

	#[test]
	fn ask_seller_is_not_nft_owner_fails() {
		new_test_ext().execute_with(|| {
			assert_ok!(Migration::force_set_migrator(RuntimeOrigin::root(), account(1)));
			mint_item(0, account(1));
			assert_noop!(
				Migration::create_ask(
					RuntimeOrigin::signed(account(1)),
					0,
					0,
					Ask {
						seller: account(2),
						price: 10000,
						expiration: 10000,
						fee: 1,
						escrow_agent: None
					}
				),
				Error::<Test>::SellerNotItemOwner
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
	fn pot_has_not_enough_funds_fails() {
		new_test_ext().execute_with(|| {
			assert_ok!(Migration::force_set_migrator(RuntimeOrigin::root(), account(1)));
			assert_noop!(
				Migration::send_funds_from_pot(
					RuntimeOrigin::signed(account(1)),
					account(2),
					10000
				),
				ArithmeticError::Underflow
			);
		})
	}

	#[test]
	fn send_funds_from_pot_passes() {
		new_test_ext().execute_with(|| {
			assert_ok!(Migration::force_set_migrator(RuntimeOrigin::root(), account(1)));
			let pot = Migration::pot_account_id();
			Balances::set_balance(&pot, 100000);

			let res = Migration::send_funds_from_pot(
				RuntimeOrigin::signed(account(1)),
				account(2),
				10000,
			);
			assert!(res.is_ok());
			assert!(res.unwrap().pays_fee == Pays::No);

			assert!(Balances::free_balance(&pot) == 90000);
			assert!(Balances::free_balance(&account(2)) == 10000);
		})
	}
}

mod set_item_owner {
	use super::*;

	#[test]
	fn sender_is_not_migrator_fails() {
		new_test_ext().execute_with(|| {
			assert_ok!(Migration::force_set_migrator(RuntimeOrigin::root(), account(1)));
			mint_item(0, account(1));
			assert_noop!(
				Migration::set_item_owner(RuntimeOrigin::signed(account(2)), 0, 0, account(2)),
				Error::<Test>::NotMigrator
			);
		})
	}

	#[test]
	fn item_not_found_fails() {
		new_test_ext().execute_with(|| {
			assert_ok!(Migration::force_set_migrator(RuntimeOrigin::root(), account(1)));
			assert_noop!(
				Migration::set_item_owner(RuntimeOrigin::signed(account(1)), 0, 0, account(2)),
				Error::<Test>::ItemNotFound
			);
		})
	}

	#[test]
	fn already_owner_fails() {
		new_test_ext().execute_with(|| {
			assert_ok!(Migration::force_set_migrator(RuntimeOrigin::root(), account(1)));
			mint_item(0, account(1));
			assert_noop!(
				Migration::set_item_owner(RuntimeOrigin::signed(account(1)), 0, 0, account(1)),
				Error::<Test>::AlreadyOwner
			);
		})
	}

	#[test]
	fn set_item_owner_passes() {
		new_test_ext().execute_with(|| {
			assert_ok!(Migration::force_set_migrator(RuntimeOrigin::root(), account(1)));
			mint_item(0, account(1));

			let res =
				Migration::set_item_owner(RuntimeOrigin::signed(account(1)), 0, 0, account(2));
			assert!(res.is_ok());
			assert!(res.unwrap().pays_fee == Pays::No);

			assert!(Nfts::owner(0, 0) == Some(account(2)));
		})
	}
}

mod force_create {

	use super::*;

	#[test]
	fn sender_is_not_migrator_fails() {
		new_test_ext().execute_with(|| {
			assert_ok!(Migration::force_set_migrator(RuntimeOrigin::root(), account(1)));
			assert_noop!(
				Migration::force_create(
					RuntimeOrigin::signed(account(2)),
					account(3),
					collection_config_with_all_settings_enabled()
				),
				Error::<Test>::NotMigrator
			);
		})
	}

	#[test]
	fn force_create_redispatch_works() {
		new_test_ext().execute_with(|| {
			assert_ok!(Migration::force_set_migrator(RuntimeOrigin::root(), account(2)));

			let res = Migration::force_create(
				RuntimeOrigin::signed(account(2)),
				account(3),
				collection_config_with_all_settings_enabled(),
			);
			assert!(res.is_ok());
			assert!(res.unwrap().pays_fee == Pays::No)
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
					0,
					Some(account(3)),
					Some(account(3)),
					Some(account(3))
				),
				Error::<Test>::NotMigrator
			);
		})
	}

	#[test]
	fn set_team_redispatch_works() {
		new_test_ext().execute_with(|| {
			assert_ok!(Migration::force_set_migrator(RuntimeOrigin::root(), account(2)));
			assert_ok!(Migration::force_create(
				RuntimeOrigin::signed(account(2)),
				account(3),
				collection_config_with_all_settings_enabled()
			));

			let res = Migration::set_team(
				RuntimeOrigin::signed(account(2)),
				0,
				Some(account(3)),
				Some(account(3)),
				Some(account(3)),
			);
			assert!(res.is_ok());
			assert!(res.unwrap().pays_fee == Pays::No)
		})
	}
}

mod set_collection_metadata {
	use sp_runtime::BoundedVec;

	use super::*;

	#[test]
	fn sender_is_not_migrator_fails() {
		new_test_ext().execute_with(|| {
			assert_ok!(Migration::force_set_migrator(RuntimeOrigin::root(), account(1)));
			assert_noop!(
				Migration::set_collection_metadata(
					RuntimeOrigin::signed(account(2)),
					0,
					BoundedVec::new()
				),
				Error::<Test>::NotMigrator
			);
		})
	}

	#[test]
	fn set_collection_metadata_redispatch_works() {
		new_test_ext().execute_with(|| {
			assert_ok!(Migration::force_set_migrator(RuntimeOrigin::root(), account(2)));
			assert_ok!(Migration::force_create(
				RuntimeOrigin::signed(account(2)),
				account(3),
				collection_config_with_all_settings_enabled()
			));

			let res = Migration::set_collection_metadata(
				RuntimeOrigin::signed(account(2)),
				0,
				BoundedVec::new(),
			);
			assert!(res.is_ok());
			assert!(res.unwrap().pays_fee == Pays::No)
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
					0,
					account(3),
					default_item_config()
				),
				Error::<Test>::NotMigrator
			);
		})
	}

	#[test]
	fn force_mint_redispatch_works() {
		new_test_ext().execute_with(|| {
			assert_ok!(Migration::force_set_migrator(RuntimeOrigin::root(), account(2)));
			assert_ok!(Migration::force_create(
				RuntimeOrigin::signed(account(2)),
				account(3),
				collection_config_with_all_settings_enabled()
			));

			let res = Migration::force_mint(
				RuntimeOrigin::signed(account(2)),
				0,
				0,
				account(3),
				default_item_config(),
			);
			assert!(res.is_ok());
			assert!(res.unwrap().pays_fee == Pays::No)
		})
	}
}
