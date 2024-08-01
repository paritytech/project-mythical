use account::{AccountId20, EthereumSignature, EthereumSigner};

use frame_support::{
	assert_noop, assert_ok,
	error::BadOrigin,
	traits::fungible::{Inspect as InspectFungible, Mutate},
};
use frame_system::pallet_prelude::BlockNumberFor;
use pallet_nfts::{CollectionConfig, CollectionSettings, MintSettings, NextCollectionId};
use sp_core::{
	ecdsa::{Pair as KeyPair, Signature},
	keccak_256, Pair,
};

use sp_runtime::traits::IdentifyAccount;

use self::mock::Timestamp;
use crate::{mock::*, *};

type AccountIdOf<Test> = <Test as frame_system::Config>::AccountId;
type CollectionId<Test> = <Test as pallet_nfts::Config>::CollectionId;
type Balance<Test> = <Test as pallet_balances::Config>::Balance;

fn account(id: u8) -> AccountIdOf<Test> {
	[id; 20].into()
}

fn create_collection(creator: &AccountIdOf<Test>) -> CollectionId<Test> {
	let collection_id = NextCollectionId::<Test>::get().unwrap_or_default();
	assert_ok!(Nfts::force_create(
		RuntimeOrigin::root(),
		*creator,
		collection_config_with_all_settings_enabled()
	));

	collection_id
}

fn collection_config_with_all_settings_enabled(
) -> CollectionConfig<Balance<Test>, BlockNumberFor<Test>, CollectionId<Test>> {
	CollectionConfig {
		settings: CollectionSettings::all_enabled(),
		max_supply: Some(1000000),
		mint_settings: MintSettings::default(),
	}
}

mod force_set_collection {
	use super::*;

	#[test]
	fn force_set_collection_works() {
		new_test_ext().execute_with(|| {
			let collection_id = create_collection(&account(0));

			assert_ok!(Dmarket::force_set_collection(RuntimeOrigin::root(), collection_id));
			assert!(DmarketCollection::<Test>::get() == Some(collection_id));

			assert_noop!(
				Dmarket::force_set_collection(RuntimeOrigin::root(), collection_id),
				Error::<Test>::CollectionAlreadyInUse
			);

			let other_collection = create_collection(&account(0));
			assert_ok!(Dmarket::force_set_collection(RuntimeOrigin::root(), other_collection));
			assert!(DmarketCollection::<Test>::get() == Some(other_collection));
		})
	}

	#[test]
	fn fails_no_root() {
		new_test_ext().execute_with(|| {
			let collection_id = create_collection(&account(0));

			assert_noop!(
				Dmarket::force_set_collection(RuntimeOrigin::signed(account(1)), collection_id),
				BadOrigin
			);
		})
	}

	#[test]
	fn collection_not_found() {
		new_test_ext().execute_with(|| {
			assert_noop!(
				Dmarket::force_set_collection(RuntimeOrigin::root(), 0),
				Error::<Test>::CollectionNotFound
			);
		})
	}
}

mod execute_trade {
	use super::*;

	fn get_trade_accounts() -> (AccountIdOf<Test>, AccountIdOf<Test>, KeyPair, KeyPair) {
		let sender = account(0);
		let fee_address = account(1);

		let seller_pair = Pair::from_string("//Seller", None).unwrap();
		let buyer_pair = Pair::from_string("//Buyer", None).unwrap();
		(sender, fee_address, seller_pair, buyer_pair)
	}

	fn setup_nft(
		sender: &AccountIdOf<Test>,
		item_owner: &AccountIdOf<Test>,
		item: u128,
	) -> CollectionId<Test> {
		let collection_id = create_collection(&sender);
		assert_ok!(Dmarket::force_set_collection(RuntimeOrigin::root(), collection_id));

		assert_ok!(Nfts::mint(RuntimeOrigin::signed(*sender), 0, Some(item), *item_owner, None));
		collection_id
	}

	fn sign_trade(
		caller: &AccountIdOf<Test>,
		fee_address: &AccountIdOf<Test>,
		trade: &TradeParamsOf<Test>,
		seller_pair: KeyPair,
		buyer_pair: KeyPair,
	) -> TradeSignaturesOf<Test> {
		let ask_message: Vec<u8> = Dmarket::get_ask_message(caller, fee_address, trade);
		let hashed_ask = keccak_256(&ask_message);

		let bid_message: Vec<u8> = Dmarket::get_bid_message(caller, fee_address, trade);
		let hashed_bid = keccak_256(&bid_message);

		TradeSignatures {
			ask_signature: EthereumSignature::from(seller_pair.sign_prehashed(&hashed_ask)),
			bid_signature: EthereumSignature::from(buyer_pair.sign_prehashed(&hashed_bid)),
		}
	}

	#[test]
	fn execute_trade_works() {
		new_test_ext().execute_with(|| {
			let (sender, fee_address, seller_pair, buyer_pair) = get_trade_accounts();

			let seller: AccountId20 = EthereumSigner::from(seller_pair.public()).into_account();
			let buyer: AccountId20 = EthereumSigner::from(buyer_pair.public()).into_account();

			let item = 1;
			let collection = setup_nft(&sender, &seller, item);

			let expiration = Timestamp::get() + 10;
			let price = 10000000;
			let fee = 100;
			Balances::set_balance(&buyer, price * 2);

			let buyer_balance = Balances::balance(&buyer);
			let fee_address_balance = Balances::balance(&fee_address);
			let seller_balance = Balances::balance(&seller);

			let trade = TradeParams {
				price,
				fee,
				item,
				ask_expiration: expiration,
				bid_expiration: expiration,
			};
			let signatures = sign_trade(&sender, &fee_address, &trade, seller_pair, buyer_pair);

			assert_ok!(Dmarket::execute_trade(
				RuntimeOrigin::signed(sender),
				seller,
				buyer,
				trade.clone(),
				signatures,
				fee_address
			));

			assert_eq!(Nfts::owner(collection, item).unwrap(), buyer);
			assert_eq!(Balances::balance(&buyer), buyer_balance - price);
			assert_eq!(Balances::balance(&seller), seller_balance + price - fee);
			assert_eq!(Balances::balance(&fee_address), fee_address_balance + fee);

			let (ask_hash, bid_hash) = Dmarket::hash_ask_bid_data(&trade);
			assert!(ClosedAsks::<Test>::contains_key(ask_hash));
			assert!(ClosedBids::<Test>::contains_key(bid_hash));
		})
	}

	#[test]
	fn buyer_is_seller() {
		new_test_ext().execute_with(|| {
			let (sender, fee_address, seller_pair, buyer_pair) = get_trade_accounts();

			let seller: AccountId20 = EthereumSigner::from(seller_pair.public()).into_account();
			let buyer: AccountId20 = seller.clone();

			let item = 1;
			let _ = setup_nft(&sender, &seller, item);

			let expiration = Timestamp::get() + 10;
			let price = 10000000;
			let fee = 100;
			Balances::set_balance(&buyer, price * 2);

			let trade = TradeParams {
				price,
				fee,
				item,
				ask_expiration: expiration,
				bid_expiration: expiration,
			};
			let signatures = sign_trade(&sender, &fee_address, &trade, seller_pair, buyer_pair);

			assert_noop!(
				Dmarket::execute_trade(
					RuntimeOrigin::signed(sender),
					seller,
					buyer,
					trade.clone(),
					signatures,
					fee_address
				),
				Error::<Test>::BuyerIsSeller
			);
		})
	}

	#[test]
	fn ask_or_bid_expired() {
		new_test_ext().execute_with(|| {
			let (sender, fee_address, seller_pair, buyer_pair) = get_trade_accounts();

			let seller: AccountId20 = EthereumSigner::from(seller_pair.public()).into_account();
			let buyer: AccountId20 = EthereumSigner::from(buyer_pair.public()).into_account();

			let item = 1;
			let _ = setup_nft(&sender, &seller, item);

			let expiration = Timestamp::get() + 10;
			let price = 10000000;
			let fee = 100;
			Balances::set_balance(&buyer, price * 2);

			let mut trade =
				TradeParams { price, fee, item, ask_expiration: 0, bid_expiration: expiration };
			let signatures =
				sign_trade(&sender, &fee_address, &trade, seller_pair.clone(), buyer_pair.clone());

			assert_noop!(
				Dmarket::execute_trade(
					RuntimeOrigin::signed(sender),
					seller,
					buyer,
					trade.clone(),
					signatures,
					fee_address
				),
				Error::<Test>::AskExpired
			);

			trade.ask_expiration = expiration;
			trade.bid_expiration = 0;
			let signatures = sign_trade(&sender, &fee_address, &trade, seller_pair, buyer_pair);
			assert_noop!(
				Dmarket::execute_trade(
					RuntimeOrigin::signed(seller),
					seller,
					buyer,
					trade.clone(),
					signatures,
					fee_address
				),
				Error::<Test>::BidExpired
			);
		})
	}

	#[test]
	fn collection_not_set() {
		new_test_ext().execute_with(|| {
			let (sender, fee_address, seller_pair, buyer_pair) = get_trade_accounts();

			let seller: AccountId20 = EthereumSigner::from(seller_pair.public()).into_account();
			let buyer: AccountId20 = EthereumSigner::from(buyer_pair.public()).into_account();

			let item = 1;

			let expiration = Timestamp::get() + 10;
			let price = 10000000;
			let fee = 100;
			Balances::set_balance(&buyer, price * 2);

			let trade = TradeParams {
				price,
				fee,
				item,
				ask_expiration: expiration,
				bid_expiration: expiration,
			};
			let signatures = sign_trade(&sender, &fee_address, &trade, seller_pair, buyer_pair);

			assert_noop!(
				Dmarket::execute_trade(
					RuntimeOrigin::signed(sender),
					seller,
					buyer,
					trade.clone(),
					signatures,
					fee_address
				),
				Error::<Test>::CollectionNotSet
			);
		})
	}

	#[test]
	fn item_not_found() {
		new_test_ext().execute_with(|| {
			let (sender, fee_address, seller_pair, buyer_pair) = get_trade_accounts();

			let seller: AccountId20 = EthereumSigner::from(seller_pair.public()).into_account();
			let buyer: AccountId20 = EthereumSigner::from(buyer_pair.public()).into_account();

			let item = 1;
			let _ = setup_nft(&sender, &seller, item);

			let expiration = Timestamp::get() + 10;
			let price = 10000000;
			let fee = 100;
			Balances::set_balance(&buyer, price * 2);

			let trade = TradeParams {
				price,
				fee,
				item: item + 1,
				ask_expiration: expiration,
				bid_expiration: expiration,
			};
			let signatures = sign_trade(&sender, &fee_address, &trade, seller_pair, buyer_pair);

			assert_noop!(
				Dmarket::execute_trade(
					RuntimeOrigin::signed(sender),
					seller,
					buyer,
					trade.clone(),
					signatures,
					fee_address
				),
				Error::<Test>::ItemNotFound
			);
		})
	}

	#[test]
	fn already_executed() {
		new_test_ext().execute_with(|| {
			let (sender, fee_address, seller_pair, buyer_pair) = get_trade_accounts();

			let seller: AccountId20 = EthereumSigner::from(seller_pair.public()).into_account();
			let buyer: AccountId20 = EthereumSigner::from(buyer_pair.public()).into_account();

			let item = 1;
			let collection = setup_nft(&sender, &seller, item);

			let expiration = Timestamp::get() + 10;
			let price = 10000000;
			let fee = 100;
			Balances::set_balance(&buyer, price * 2);

			let trade = TradeParams {
				price,
				fee,
				item,
				ask_expiration: expiration,
				bid_expiration: expiration,
			};
			let signatures = sign_trade(&sender, &fee_address, &trade, seller_pair, buyer_pair);

			assert_ok!(Dmarket::execute_trade(
				RuntimeOrigin::signed(sender),
				seller,
				buyer,
				trade.clone(),
				signatures.clone(),
				fee_address
			));

			assert_ok!(Nfts::transfer(RuntimeOrigin::signed(buyer), collection, item, seller));
			assert_noop!(
				Dmarket::execute_trade(
					RuntimeOrigin::signed(sender),
					seller,
					buyer,
					trade.clone(),
					signatures.clone(),
					fee_address
				),
				Error::<Test>::AskAlreadyExecuted
			);

			let (ask_hash, _) = Dmarket::hash_ask_bid_data(&trade);
			ClosedAsks::<Test>::remove(ask_hash);
			assert_noop!(
				Dmarket::execute_trade(
					RuntimeOrigin::signed(sender),
					seller,
					buyer,
					trade.clone(),
					signatures,
					fee_address
				),
				Error::<Test>::BidAlreadyExecuted
			);
		})
	}

	#[test]
	fn invalid_signatures() {
		new_test_ext().execute_with(|| {
			let (sender, fee_address, seller_pair, buyer_pair) = get_trade_accounts();

			let seller: AccountId20 = EthereumSigner::from(seller_pair.public()).into_account();
			let buyer: AccountId20 = EthereumSigner::from(buyer_pair.public()).into_account();

			let item = 1;
			let _ = setup_nft(&sender, &seller, item);

			let expiration = Timestamp::get() + 10;
			let price = 10000000;
			let fee = 100;
			Balances::set_balance(&buyer, price * 2);

			let trade = TradeParams {
				price,
				fee,
				item,
				ask_expiration: expiration,
				bid_expiration: expiration,
			};
			let mut signatures =
				sign_trade(&sender, &fee_address, &trade, seller_pair.clone(), buyer_pair.clone());
			signatures.ask_signature = EthereumSignature::from(Signature::from_raw([0; 65]));

			assert_noop!(
				Dmarket::execute_trade(
					RuntimeOrigin::signed(sender),
					seller,
					buyer,
					trade.clone(),
					signatures,
					fee_address
				),
				Error::<Test>::InvalidSellerSignature
			);

			let mut signatures = sign_trade(&sender, &fee_address, &trade, seller_pair, buyer_pair);
			signatures.bid_signature = EthereumSignature::from(Signature::from_raw([0; 65]));

			assert_noop!(
				Dmarket::execute_trade(
					RuntimeOrigin::signed(sender),
					seller,
					buyer,
					trade.clone(),
					signatures,
					fee_address
				),
				Error::<Test>::InvalidBuyerSignature
			);
		})
	}

	#[test]
	fn seller_not_owner() {
		new_test_ext().execute_with(|| {
			let (sender, fee_address, seller_pair, buyer_pair) = get_trade_accounts();

			let seller: AccountId20 = EthereumSigner::from(seller_pair.public()).into_account();
			let buyer: AccountId20 = EthereumSigner::from(buyer_pair.public()).into_account();

			let item = 1;
			let _ = setup_nft(&sender, &account(1), item);

			let expiration = Timestamp::get() + 10;
			let price = 10000000;
			let fee = 100;
			Balances::set_balance(&buyer, price * 2);

			let trade = TradeParams {
				price,
				fee,
				item,
				ask_expiration: expiration,
				bid_expiration: expiration,
			};
			let signatures = sign_trade(&sender, &fee_address, &trade, seller_pair, buyer_pair);

			assert_noop!(
				Dmarket::execute_trade(
					RuntimeOrigin::signed(sender),
					seller,
					buyer,
					trade.clone(),
					signatures,
					fee_address
				),
				Error::<Test>::SellerNotItemOwner
			);
		})
	}

	#[test]
	fn buyer_not_enough_funds() {
		new_test_ext().execute_with(|| {
			let (sender, fee_address, seller_pair, buyer_pair) = get_trade_accounts();

			let seller: AccountId20 = EthereumSigner::from(seller_pair.public()).into_account();
			let buyer: AccountId20 = EthereumSigner::from(buyer_pair.public()).into_account();

			let item = 1;
			let _ = setup_nft(&sender, &seller, item);

			let expiration = Timestamp::get() + 10;
			let price = 10000000;
			let fee = 100;
			Balances::set_balance(&buyer, price - 10);

			let trade = TradeParams {
				price,
				fee,
				item,
				ask_expiration: expiration,
				bid_expiration: expiration,
			};
			let signatures = sign_trade(&sender, &fee_address, &trade, seller_pair, buyer_pair);

			assert_noop!(
				Dmarket::execute_trade(
					RuntimeOrigin::signed(sender),
					seller,
					buyer,
					trade.clone(),
					signatures,
					fee_address
				),
				Error::<Test>::BuyerBalanceTooLow
			);
		})
	}
}
