use crate::{mock::*, *};
use frame_support::{
	assert_noop, assert_ok,
	error::BadOrigin,
	traits::{
		fungible::{Inspect as InspectFungible, InspectHold, Mutate},
		nonfungibles_v2::{Inspect, Transfer},
	},
};
use frame_system::pallet_prelude::BlockNumberFor;
use pallet_nfts::{CollectionConfig, CollectionSettings, MintSettings};
use parity_scale_codec::Encode;
use sp_core::{
	ecdsa::{Pair as KeyPair, Signature},
	Get, Pair,
};
use sp_runtime::{traits::IdentifyAccount, BoundedVec, MultiSignature};
use account::{EthereumSignature, EthereumSigner};
use self::mock::Timestamp;


type AccountIdOf<Test> = <Test as frame_system::Config>::AccountId;
type CollectionId<Test> = <Test as pallet_nfts::Config>::CollectionId;
type OffchainSignature<Test> = <Test as pallet_nfts::Config>::OffchainSignature;
type ItemId<Test> = <Test as pallet_nfts::Config>::ItemId;
type Moment<Test> = <Test as pallet_timestamp::Config>::Moment;
type Balance<Test> = <Test as pallet_balances::Config>::Balance;

fn account(id: u8) -> AccountIdOf<Test> {
	[id; 20].into()
}

fn admin_accounts_setup() -> (AccountIdOf<Test>, KeyPair) {
	let admin_pair = sp_core::ecdsa::Pair::from_string("//Alice", None).unwrap();
	let admin_signer: EthereumSigner = admin_pair.public().into();
	let admin = admin_signer.clone().into_account();
	assert_ok!(Marketplace::force_set_authority(RuntimeOrigin::root(), admin.clone()));
	assert_ok!(Marketplace::set_fee_signer_address(
		RuntimeOrigin::signed(admin.clone()),
		admin.clone()
	));
	assert_ok!(Marketplace::set_payout_address(
		RuntimeOrigin::signed(admin.clone()),
		admin.clone()
	));

	(admin, admin_pair)
}

fn get_valid_expiration() -> Moment<Test> {
	let timestamp: Moment<Test> = Timestamp::get();
	let min_order_duration: Moment<Test> = <Test as Config>::MinOrderDuration::get();

	timestamp + min_order_duration + 1
}

fn collection_config_with_all_settings_enabled(
) -> CollectionConfig<Balance<Test>, BlockNumberFor<Test>, CollectionId<Test>> {
	CollectionConfig {
		settings: CollectionSettings::all_enabled(),
		max_supply: None,
		mint_settings: MintSettings::default(),
	}
}

fn append_valid_signature(
	fee_signer_pair: KeyPair,
	order: &mut Order<
		CollectionId<Test>,
		ItemId<Test>,
		BalanceOf<Test>,
		Moment<Test>,
		OffchainSignature<Test>,
		Vec<u8>,
	>,
) {
	let message = (
		order.order_type.clone(),
		order.collection,
		order.item,
		order.price,
		order.expires_at,
		order.fee_percent,
		order.signature_data.nonce.clone(),
	)
		.encode();

	let mut wrapped_data: Vec<u8> = Vec::new();
	wrapped_data.extend(b"\x19Ethereum Signed Message:\n32");
	wrapped_data.extend(&message);

	let signature = EthereumSignature::from(MultiSignature::Ecdsa(fee_signer_pair.sign(&wrapped_data)));
	order.signature_data.signature = signature;
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

pub fn raw_signature(bytes: [u8;65]) -> EthereumSignature {
	EthereumSignature::from(MultiSignature::Ecdsa(Signature::from_raw(bytes)))
}

pub fn create_valid_order(
	order_type: OrderType,
	who: AccountIdOf<Test>,
	item_owner: AccountIdOf<Test>,
) {
	let fee_signer_pair = sp_core::ecdsa::Pair::from_string("//Alice", None).unwrap();
	let expires_at = get_valid_expiration();
	let max_basis_points: Balance<Test> = <Test as Config>::MaxBasisPoints::get();
	mint_item(0, item_owner);

	if order_type.clone() == OrderType::Bid {
		Balances::set_balance(&who, 100000);
	}

	let mut order = Order {
		order_type,
		collection: 0,
		item: 0,
		expires_at,
		price: 10000,
		fee_percent: max_basis_points,
		signature_data: SignatureData {
			signature: raw_signature([0; 65]),
			nonce: <Vec<u8>>::new(),
		},
	};
	append_valid_signature(fee_signer_pair, &mut order);

	assert_ok!(Marketplace::create_order(
		RuntimeOrigin::signed(who),
		order.clone(),
		Execution::AllowCreation
	));
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

mod create_order_initial_checks {
	use super::*;
	use sp_core::ConstU32;

	#[test]
	fn item_not_found() {
		new_test_ext().execute_with(|| {
			let expires_at = get_valid_expiration();
			let max_basis_points: u128 = <Test as Config>::MaxBasisPoints::get();
			let (_, fee_signer_pair) = admin_accounts_setup();

			let mut order = Order {
				order_type: OrderType::Ask,
				collection: 0,
				item: 0,
				expires_at,
				price: max_basis_points + 1,
				fee_percent: 1,
				signature_data: SignatureData {
					signature: raw_signature([0; 65]),
					nonce: <Vec<u8>>::new(),
				},
			};
			append_valid_signature(fee_signer_pair, &mut order);

			assert_noop!(
				Marketplace::create_order(
					RuntimeOrigin::signed(account(1)),
					order,
					Execution::AllowCreation
				),
				Error::<Test>::ItemNotFound
			);
		})
	}

	#[test]
	fn invalid_order_price() {
		new_test_ext().execute_with(|| {
			let expires_at = get_valid_expiration();
			let max_basis_points: u128 = <Test as Config>::MaxBasisPoints::get();
			mint_item(0, account(1));

			let (_, fee_signer_pair) = admin_accounts_setup();

			let mut order = Order {
				order_type: OrderType::Ask,
				collection: 0,
				item: 0,
				expires_at,
				price: max_basis_points - 1,
				fee_percent: 1,
				signature_data: SignatureData {
					signature: raw_signature([0; 65]),
					nonce: <Vec<u8>>::new(),
				},
			};
			append_valid_signature(fee_signer_pair, &mut order);

			assert_noop!(
				Marketplace::create_order(
					RuntimeOrigin::signed(account(1)),
					order,
					Execution::AllowCreation
				),
				Error::<Test>::InvalidPrice
			);
		})
	}

	#[test]
	fn invalid_expires_at() {
		new_test_ext().execute_with(|| {
			let timestamp: u64 = Timestamp::get();
			let min_order_duration: u64 = <Test as Config>::MinOrderDuration::get();
			mint_item(0, account(1));

			let (_, fee_signer_pair) = admin_accounts_setup();

			let mut order = Order {
				order_type: OrderType::Ask,
				collection: 0,
				item: 0,
				expires_at: timestamp + min_order_duration,
				price: 10000,
				fee_percent: 1,
				signature_data: SignatureData {
					signature: raw_signature([0; 65]),
					nonce: <Vec<u8>>::new(),
				},
			};
			append_valid_signature(fee_signer_pair, &mut order);

			assert_noop!(
				Marketplace::create_order(
					RuntimeOrigin::signed(account(1)),
					order,
					Execution::AllowCreation
				),
				Error::<Test>::InvalidExpiration
			);
		})
	}

	// TODO: Order creation should fail if the signature is invalid - BadSignedMessage
	#[test]
	fn invalid_signed_message() {
		new_test_ext().execute_with(|| {
			let expires_at = get_valid_expiration();
			let max_basis_points: u128 = <Test as Config>::MaxBasisPoints::get();
			mint_item(0, account(1));

			let _ = admin_accounts_setup();

			let order = Order {
				order_type: OrderType::Ask,
				collection: 0,
				item: 0,
				expires_at,
				price: max_basis_points + 1,
				fee_percent: 1,
				signature_data: SignatureData {
					signature: raw_signature([0; 65]),
					nonce: <Vec<u8>>::new(),
				},
			};

			assert_noop!(
				Marketplace::create_order(
					RuntimeOrigin::signed(account(1)),
					order,
					Execution::AllowCreation
				),
				Error::<Test>::BadSignedMessage
			);
		})
	}

	#[test]
	fn invalid_fee_percent() {
		new_test_ext().execute_with(|| {
			let expires_at = get_valid_expiration();
			let max_basis_points: u128 = <Test as Config>::MaxBasisPoints::get();
			mint_item(0, account(1));

			let (_, fee_signer_pair) = admin_accounts_setup();

			let mut order = Order {
				order_type: OrderType::Ask,
				collection: 0,
				item: 0,
				expires_at,
				price: 10000,
				fee_percent: max_basis_points + 1,
				signature_data: SignatureData {
					signature: raw_signature([0; 65]),
					nonce: <Vec<u8>>::new(),
				},
			};
			append_valid_signature(fee_signer_pair, &mut order);

			assert_noop!(
				Marketplace::create_order(
					RuntimeOrigin::signed(account(1)),
					order,
					Execution::AllowCreation
				),
				Error::<Test>::InvalidFeePercent
			);
		})
	}

	#[test]
	fn fee_signer_nonce_already_used() {
		new_test_ext().execute_with(|| {
			let nonce: BoundedVec<u8, ConstU32<50>> = vec![0u8].try_into().unwrap();
			Nonces::<Test>::set(nonce.clone(), true);

			let (_, fee_signer_pair) = admin_accounts_setup();

			let expires_at = get_valid_expiration();
			let max_basis_points: u128 = <Test as Config>::MaxBasisPoints::get();
			mint_item(0, account(1));

			let mut order = Order {
				order_type: OrderType::Ask,
				collection: 0,
				item: 0,
				expires_at,
				price: 10000,
				fee_percent: max_basis_points,
				signature_data: SignatureData {
					signature: raw_signature([0; 65]),
					nonce: vec![0u8],
				},
			};
			append_valid_signature(fee_signer_pair, &mut order);

			assert_noop!(
				Marketplace::create_order(
					RuntimeOrigin::signed(account(1)),
					order,
					Execution::AllowCreation
				),
				Error::<Test>::AlreadyUsedNonce
			);
		})
	}
}

mod create_ask {
	use super::*;

	#[test]
	fn ask_not_item_owner() {
		new_test_ext().execute_with(|| {
			let expires_at = get_valid_expiration();
			let max_basis_points: u128 = <Test as Config>::MaxBasisPoints::get();
			mint_item(0, account(1));

			let (_, fee_signer_pair) = admin_accounts_setup();

			let mut order = Order {
				order_type: OrderType::Ask,
				collection: 0,
				item: 0,
				expires_at,
				price: 10000,
				fee_percent: max_basis_points,
				signature_data: SignatureData {
					signature: raw_signature([0; 65]),
					nonce: <Vec<u8>>::new(),
				},
			};
			append_valid_signature(fee_signer_pair, &mut order);

			assert_noop!(
				Marketplace::create_order(
					RuntimeOrigin::signed(account(2)),
					order,
					Execution::AllowCreation
				),
				Error::<Test>::NotItemOwner
			);
		})
	}

	#[test]
	fn ask_item_locked() {
		new_test_ext().execute_with(|| {
			let expires_at = get_valid_expiration();
			let max_basis_points: u128 = <Test as Config>::MaxBasisPoints::get();
			mint_item(0, account(1));
			Nfts::disable_transfer(&0, &0).unwrap();

			let (_, fee_signer_pair) = admin_accounts_setup();

			let mut order = Order {
				order_type: OrderType::Ask,
				collection: 0,
				item: 0,
				expires_at,
				price: 10000,
				fee_percent: max_basis_points,
				signature_data: SignatureData {
					signature: raw_signature([0; 65]),
					nonce: <Vec<u8>>::new(),
				},
			};
			append_valid_signature(fee_signer_pair, &mut order);

			assert_noop!(
				Marketplace::create_order(
					RuntimeOrigin::signed(account(1)),
					order,
					Execution::AllowCreation
				),
				Error::<Test>::ItemAlreadyLocked
			);
		})
	}

	#[test]
	fn ask_created_with_allow_creation() {
		new_test_ext().execute_with(|| {
			let expires_at = get_valid_expiration();
			let max_basis_points: u128 = <Test as Config>::MaxBasisPoints::get();
			mint_item(0, account(1));

			let (_, fee_signer_pair) = admin_accounts_setup();

			let mut order = Order {
				order_type: OrderType::Ask,
				collection: 0,
				item: 0,
				expires_at,
				price: 10000,
				fee_percent: max_basis_points,
				signature_data: SignatureData {
					signature: raw_signature([0; 65]),
					nonce: <Vec<u8>>::new(),
				},
			};
			append_valid_signature(fee_signer_pair, &mut order);

			assert_ok!(Marketplace::create_order(
				RuntimeOrigin::signed(account(1)),
				order.clone(),
				Execution::AllowCreation
			));

			let ask = Ask {
				seller: account(1),
				price: order.price,
				expiration: order.expires_at,
				fee: order.fee_percent,
			};

			assert!(Marketplace::asks(0, 0) == Some(ask));
			assert!(!Nfts::can_transfer(&0, &0));
		})
	}

	#[test]
	fn ask_should_not_create_with_execution_force() {
		new_test_ext().execute_with(|| {
			let expires_at = get_valid_expiration();
			let max_basis_points: u128 = <Test as Config>::MaxBasisPoints::get();
			mint_item(0, account(1));

			let (_, fee_signer_pair) = admin_accounts_setup();

			let mut order = Order {
				order_type: OrderType::Ask,
				collection: 0,
				item: 0,
				expires_at,
				price: 10000,
				fee_percent: max_basis_points,
				signature_data: SignatureData {
					signature: raw_signature([0; 65]),
					nonce: <Vec<u8>>::new(),
				},
			};
			append_valid_signature(fee_signer_pair, &mut order);

			assert_noop!(
				Marketplace::create_order(
					RuntimeOrigin::signed(account(1)),
					order.clone(),
					Execution::Force
				),
				Error::<Test>::ValidMatchMustExist
			);
		})
	}

	#[test]
	fn ask_already_exists() {
		new_test_ext().execute_with(|| {
			let (_, fee_signer_pair) = admin_accounts_setup();

			create_valid_order(OrderType::Ask, account(1), account(1));

			let expires_at = get_valid_expiration();
			let max_basis_points: u128 = <Test as Config>::MaxBasisPoints::get();

			let mut order = Order {
				order_type: OrderType::Ask,
				collection: 0,
				item: 0,
				expires_at,
				price: 10000,
				fee_percent: max_basis_points,
				signature_data: SignatureData {
					signature: raw_signature([0; 65]),
					nonce: vec![1],
				},
			};
			append_valid_signature(fee_signer_pair, &mut order);

			assert_noop!(
				Marketplace::create_order(
					RuntimeOrigin::signed(account(1)),
					order,
					Execution::AllowCreation
				),
				Error::<Test>::OrderAlreadyExists
			);
		})
	}
}

mod create_bid {
	use super::*;

	#[test]
	fn bid_created_with_allow_creation() {
		new_test_ext().execute_with(|| {
			let expires_at = get_valid_expiration();
			let max_basis_points: u128 = <Test as Config>::MaxBasisPoints::get();
			mint_item(0, account(2));

			let (_, fee_signer_pair) = admin_accounts_setup();

			Balances::set_balance(&account(1), 100000);
			let initial_reserved =
				Balances::balance_on_hold(&HoldReason::MarketplaceBid.into(), &account(1));

			let mut order = Order {
				order_type: OrderType::Bid,
				collection: 0,
				item: 0,
				expires_at,
				price: 10000,
				fee_percent: max_basis_points.clone(),
				signature_data: SignatureData {
					signature: raw_signature([0; 65]),
					nonce: <Vec<u8>>::new(),
				},
			};
			append_valid_signature(fee_signer_pair, &mut order);

			assert_ok!(Marketplace::create_order(
				RuntimeOrigin::signed(account(1)),
				order.clone(),
				Execution::AllowCreation
			));

			let bid =
				Bid { buyer: account(1), expiration: order.expires_at, fee: order.fee_percent };
			assert!(
				Some(
					Balances::balance_on_hold(&HoldReason::MarketplaceBid.into(), &account(1))
						.saturating_sub(initial_reserved)
				) == Marketplace::calc_bid_payment(&order.price, &order.fee_percent).ok()
			);
			assert!(Marketplace::bids((0, 0, order.price)) == Some(bid));
		})
	}

	#[test]
	fn bid_should_not_create_with_execution_force() {
		new_test_ext().execute_with(|| {
			let expires_at = get_valid_expiration();
			let max_basis_points: u128 = <Test as Config>::MaxBasisPoints::get();
			mint_item(0, account(2));

			let (_, fee_signer_pair) = admin_accounts_setup();

			Balances::set_balance(&account(1), 100000);

			let mut order = Order {
				order_type: OrderType::Bid,
				collection: 0,
				item: 0,
				expires_at,
				price: 10000,
				fee_percent: max_basis_points.clone(),
				signature_data: SignatureData {
					signature: raw_signature([0; 65]),
					nonce: <Vec<u8>>::new(),
				},
			};
			append_valid_signature(fee_signer_pair, &mut order);

			assert_noop!(
				Marketplace::create_order(
					RuntimeOrigin::signed(account(1)),
					order.clone(),
					Execution::Force
				),
				Error::<Test>::ValidMatchMustExist
			);
		})
	}

	#[test]
	fn bid_on_owned_item() {
		new_test_ext().execute_with(|| {
			let expires_at = get_valid_expiration();
			let max_basis_points: u128 = <Test as Config>::MaxBasisPoints::get();
			mint_item(0, account(1));
			Balances::set_balance(&account(1), 100000);

			let (_, fee_signer_pair) = admin_accounts_setup();

			let mut order = Order {
				order_type: OrderType::Bid,
				collection: 0,
				item: 0,
				expires_at,
				price: 10000,
				fee_percent: max_basis_points,
				signature_data: SignatureData {
					signature: raw_signature([0; 65]),
					nonce: <Vec<u8>>::new(),
				},
			};
			append_valid_signature(fee_signer_pair, &mut order);

			assert_noop!(
				Marketplace::create_order(
					RuntimeOrigin::signed(account(1)),
					order.clone(),
					Execution::AllowCreation
				),
				Error::<Test>::BidOnOwnedItem
			);
		})
	}

	#[test]
	fn bid_already_exists() {
		new_test_ext().execute_with(|| {
			let expires_at = get_valid_expiration();
			let max_basis_points: u128 = <Test as Config>::MaxBasisPoints::get();
			mint_item(0, account(2));
			Balances::set_balance(&account(1), 100000);

			let (_, fee_signer_pair) = admin_accounts_setup();

			let mut order = Order {
				order_type: OrderType::Bid,
				collection: 0,
				item: 0,
				expires_at,
				price: 10000,
				fee_percent: max_basis_points,
				signature_data: SignatureData {
					signature: raw_signature([0; 65]),
					nonce: <Vec<u8>>::new(),
				},
			};
			append_valid_signature(fee_signer_pair.clone(), &mut order);

			// Create a bid
			assert_ok!(Marketplace::create_order(
				RuntimeOrigin::signed(account(1)),
				order.clone(),
				Execution::AllowCreation
			));

			// Another account tries to create same bid
			order.signature_data.nonce = vec![1]; //set an unused nonce
			append_valid_signature(fee_signer_pair, &mut order);

			Balances::set_balance(&account(3), 1000);
			assert_noop!(
				Marketplace::create_order(
					RuntimeOrigin::signed(account(3)),
					order,
					Execution::AllowCreation
				),
				Error::<Test>::OrderAlreadyExists
			);
		})
	}

	#[test]
	fn should_calculte_bid_payment() {
		new_test_ext().execute_with(|| {
			let price = 10000;
			let fee = 2000;
			assert_eq!(Marketplace::calc_bid_payment(&price, &fee).ok(), Some(12000))
		})
	}

	#[test]
	fn bid_not_enough_balance() {
		new_test_ext().execute_with(|| {
			let expires_at = get_valid_expiration();
			let max_basis_points: u128 = <Test as Config>::MaxBasisPoints::get();
			mint_item(0, account(2));
			Balances::set_balance(&account(1), 1);

			let (_, fee_signer_pair) = admin_accounts_setup();

			let mut order = Order {
				order_type: OrderType::Bid,
				collection: 0,
				item: 0,
				expires_at,
				price: 10000000000,
				fee_percent: max_basis_points,
				signature_data: SignatureData {
					signature: raw_signature([0; 65]),
					nonce: <Vec<u8>>::new(),
				},
			};
			append_valid_signature(fee_signer_pair, &mut order);

			assert_noop!(
				Marketplace::create_order(
					RuntimeOrigin::signed(account(1)),
					order.clone(),
					Execution::AllowCreation
				),
				Error::<Test>::InsufficientFunds
			);
		})
	}
}

mod execute_ask_with_existing_bid {
	use super::*;

	#[test]
	fn order_executed() {
		new_test_ext().execute_with(|| {
			let buyer = account(2);
			let seller = account(1);

			let expires_at = get_valid_expiration();
			let max_basis_points: u128 = <Test as Config>::MaxBasisPoints::get();
			let price = 10000;

			mint_item(0, seller.clone());
			Balances::set_balance(&buyer, 100000);

			let (_, fee_signer_pair) = admin_accounts_setup();

			let bid_fee = 3;
			let mut bid = Order {
				order_type: OrderType::Bid,
				collection: 0,
				item: 0,
				expires_at,
				price,
				fee_percent: bid_fee,
				signature_data: SignatureData {
					signature: raw_signature([0; 65]),
					nonce: <Vec<u8>>::new(),
				},
			};
			append_valid_signature(fee_signer_pair.clone(), &mut bid);

			assert_ok!(Marketplace::create_order(
				RuntimeOrigin::signed(buyer.clone()),
				bid.clone(),
				Execution::AllowCreation
			));

			let payout_address = Marketplace::payout_address().unwrap();
			let payout_address_balance_before = Balances::balance(&payout_address);
			let seller_balance_before = Balances::balance(&seller);
			let buyer_reserved_balance_before =
				Balances::balance_on_hold(&HoldReason::MarketplaceBid.into(), &buyer);

			let ask_fee = 2;
			let mut order = Order {
				order_type: OrderType::Ask,
				collection: 0,
				item: 0,
				expires_at,
				price: price.clone(),
				fee_percent: ask_fee.clone(),
				signature_data: SignatureData {
					signature: raw_signature([0; 65]),
					nonce: vec![1],
				},
			};
			append_valid_signature(fee_signer_pair, &mut order);

			let buyer_fee = (price.clone() * bid_fee) / max_basis_points.clone();
			let seller_fee = (price.clone() * ask_fee) / max_basis_points.clone();
			let buyer_payment = price + buyer_fee.clone();
			let marketplace_pay = buyer_fee + seller_fee;
			let seller_pay = buyer_payment.clone() - marketplace_pay.clone();

			assert_ok!(Marketplace::create_order(
				RuntimeOrigin::signed(seller.clone()),
				order,
				Execution::AllowCreation
			));
			assert_eq!(Nfts::owner(0, 0), Some(buyer.clone()));
			assert!(Nfts::can_transfer(&0, &0));
			assert_eq!(
				payout_address_balance_before + marketplace_pay,
				Balances::balance(&payout_address)
			);
			assert_eq!(seller_balance_before + seller_pay, Balances::balance(&seller));
			assert_eq!(
				buyer_reserved_balance_before - buyer_payment,
				Balances::balance_on_hold(&HoldReason::MarketplaceBid.into(), &buyer)
			)
		})
	}

	#[test]
	fn buyer_is_seller() {
		new_test_ext().execute_with(|| {
			let (_, fee_signer_pair) = admin_accounts_setup();

			create_valid_order(OrderType::Bid, account(2), account(1));

			let mut bid = Bids::<Test>::get((0, 0, 10000)).unwrap();
			bid.buyer = account(1);
			Bids::<Test>::set((0, 0, 10000), Some(bid.clone()));

			let expires_at = get_valid_expiration();
			let max_basis_points: u128 = <Test as Config>::MaxBasisPoints::get();

			let mut order = Order {
				order_type: OrderType::Ask,
				collection: 0,
				item: 0,
				expires_at,
				price: 10000,
				fee_percent: max_basis_points,
				signature_data: SignatureData {
					signature: raw_signature([0; 65]),
					nonce: vec![1],
				},
			};
			append_valid_signature(fee_signer_pair, &mut order);

			assert_noop!(
				Marketplace::create_order(
					RuntimeOrigin::signed(account(1)),
					order,
					Execution::AllowCreation
				),
				Error::<Test>::BuyerIsSeller
			);
		})
	}

	#[test]
	fn payout_address_not_set() {
		new_test_ext().execute_with(|| {
			let buyer = account(2);
			let seller = account(1);

			let expires_at = get_valid_expiration();
			let price = 10000;

			mint_item(0, seller.clone());
			Balances::set_balance(&buyer, 100000);

			let fee_signer_pair = sp_core::ecdsa::Pair::from_string("//Alice", None).unwrap();
			let admin_signer: EthereumSigner = fee_signer_pair.public().into();
			let admin = admin_signer.clone().into_account();
			assert_ok!(Marketplace::force_set_authority(RuntimeOrigin::root(), admin.clone()));
			assert_ok!(Marketplace::set_fee_signer_address(
				RuntimeOrigin::signed(admin.clone()),
				admin.clone()
			));

			let bid_fee = 3;
			let mut bid = Order {
				order_type: OrderType::Bid,
				collection: 0,
				item: 0,
				expires_at,
				price: price.clone(),
				fee_percent: bid_fee,
				signature_data: SignatureData {
					signature: raw_signature([0; 65]),
					nonce: <Vec<u8>>::new(),
				},
			};
			append_valid_signature(fee_signer_pair.clone(), &mut bid);

			assert_ok!(Marketplace::create_order(
				RuntimeOrigin::signed(buyer.clone()),
				bid.clone(),
				Execution::AllowCreation
			));

			let ask_fee = 2;
			let mut order = Order {
				order_type: OrderType::Ask,
				collection: 0,
				item: 0,
				expires_at,
				price: price.clone(),
				fee_percent: ask_fee.clone(),
				signature_data: SignatureData {
					signature: raw_signature([0; 65]),
					nonce: vec![1],
				},
			};
			append_valid_signature(fee_signer_pair, &mut order);

			assert_noop!(
				Marketplace::create_order(
					RuntimeOrigin::signed(seller.clone()),
					order,
					Execution::AllowCreation
				),
				Error::<Test>::PayoutAddressNotSet
			);
		})
	}

	#[test]
	fn order_executed_zero_fees() {
		new_test_ext().execute_with(|| {
			let buyer = account(2);
			let seller = account(1);

			let expires_at = get_valid_expiration();
			let max_basis_points: u128 = <Test as Config>::MaxBasisPoints::get();
			let price = 10000;

			let (_, fee_signer_pair) = admin_accounts_setup();

			mint_item(0, seller.clone());
			Balances::set_balance(&buyer, 100000);

			let bid_fee = 0;
			let mut bid = Order {
				order_type: OrderType::Bid,
				collection: 0,
				item: 0,
				expires_at,
				price: price.clone(),
				fee_percent: bid_fee,
				signature_data: SignatureData {
					signature: raw_signature([0; 65]),
					nonce: <Vec<u8>>::new(),
				},
			};
			append_valid_signature(fee_signer_pair.clone(), &mut bid);

			assert_ok!(Marketplace::create_order(
				RuntimeOrigin::signed(buyer.clone()),
				bid.clone(),
				Execution::AllowCreation
			));

			let payout_address = Marketplace::payout_address().unwrap();
			let payout_address_balance_before = Balances::balance(&payout_address);
			let seller_balance_before = Balances::balance(&seller);
			let buyer_reserved_balance_before =
				Balances::balance_on_hold(&HoldReason::MarketplaceBid.into(), &buyer);

			let ask_fee = 0;
			let mut order = Order {
				order_type: OrderType::Ask,
				collection: 0,
				item: 0,
				expires_at,
				price: price.clone(),
				fee_percent: ask_fee.clone(),
				signature_data: SignatureData {
					signature: raw_signature([0; 65]),
					nonce: vec![1],
				},
			};
			append_valid_signature(fee_signer_pair, &mut order);

			let buyer_fee = (price.clone() * bid_fee) / max_basis_points.clone();
			let seller_fee = (price.clone() * ask_fee) / max_basis_points.clone();
			let buyer_payment = price + buyer_fee.clone();
			let marketplace_pay = buyer_fee + seller_fee;
			let seller_pay = buyer_payment.clone() - marketplace_pay.clone();

			assert_ok!(Marketplace::create_order(
				RuntimeOrigin::signed(seller.clone()),
				order,
				Execution::AllowCreation
			));
			assert_eq!(Nfts::owner(0, 0), Some(buyer.clone()));
			assert!(Nfts::can_transfer(&0, &0));
			assert_eq!(
				payout_address_balance_before + marketplace_pay,
				Balances::balance(&payout_address)
			);
			assert_eq!(seller_balance_before + seller_pay, Balances::balance(&seller));
			assert_eq!(
				buyer_reserved_balance_before - buyer_payment,
				Balances::balance_on_hold(&HoldReason::MarketplaceBid.into(), &buyer)
			)
		})
	}
}

mod execute_bid_with_existing_ask {
	use super::*;

	#[test]
	fn order_executed() {
		new_test_ext().execute_with(|| {
			let buyer = account(2);
			let seller = account(1);

			let expires_at = get_valid_expiration();
			let max_basis_points: u128 = <Test as Config>::MaxBasisPoints::get();
			let price = 10000;

			mint_item(0, seller.clone());

			let (_, fee_signer_pair) = admin_accounts_setup();

			let ask_fee = 2;
			let mut order = Order {
				order_type: OrderType::Ask,
				collection: 0,
				item: 0,
				expires_at,
				price: price.clone(),
				fee_percent: ask_fee.clone(),
				signature_data: SignatureData {
					signature: raw_signature([0; 65]),
					nonce: vec![1],
				},
			};
			append_valid_signature(fee_signer_pair.clone(), &mut order);

			assert_ok!(Marketplace::create_order(
				RuntimeOrigin::signed(seller.clone()),
				order,
				Execution::AllowCreation
			));

			Balances::set_balance(&buyer, 1000000);

			let payout_address = Marketplace::payout_address().unwrap();
			let payout_address_balance_before = Balances::balance(&payout_address);
			let seller_balance_before = Balances::balance(&seller);
			let buyer_balance_before = Balances::balance(&buyer);

			let bid_fee = 3;
			let mut bid = Order {
				order_type: OrderType::Bid,
				collection: 0,
				item: 0,
				expires_at,
				price: price.clone(),
				fee_percent: bid_fee,
				signature_data: SignatureData {
					signature: raw_signature([0; 65]),
					nonce: <Vec<u8>>::new(),
				},
			};
			append_valid_signature(fee_signer_pair, &mut bid);

			assert_ok!(Marketplace::create_order(
				RuntimeOrigin::signed(buyer.clone()),
				bid.clone(),
				Execution::AllowCreation
			));

			let buyer_fee = (price.clone() * bid_fee) / max_basis_points.clone();
			let seller_fee = (price.clone() * ask_fee) / max_basis_points.clone();
			let buyer_payment = price + buyer_fee.clone();
			let marketplace_pay = buyer_fee + seller_fee;
			let seller_pay = buyer_payment.clone() - marketplace_pay.clone();

			assert_eq!(Nfts::owner(0, 0), Some(buyer.clone()));
			assert!(Nfts::can_transfer(&0, &0));
			assert_eq!(
				payout_address_balance_before + marketplace_pay,
				Balances::balance(&payout_address)
			);
			assert_eq!(seller_balance_before + seller_pay, Balances::balance(&seller));
			assert_eq!(buyer_balance_before - buyer_payment, Balances::balance(&buyer))
		})
	}

	#[test]
	fn order_executed_zero_fees() {
		new_test_ext().execute_with(|| {
			let buyer = account(2);
			let seller = account(1);

			let expires_at = get_valid_expiration();
			let max_basis_points: u128 = <Test as Config>::MaxBasisPoints::get();
			let price = 100000;

			mint_item(0, seller.clone());

			let (_, fee_signer_pair) = admin_accounts_setup();

			let ask_fee = 0;
			let mut order = Order {
				order_type: OrderType::Ask,
				collection: 0,
				item: 0,
				expires_at,
				price: price.clone(),
				fee_percent: ask_fee.clone(),
				signature_data: SignatureData {
					signature: raw_signature([0; 65]),
					nonce: vec![1],
				},
			};
			append_valid_signature(fee_signer_pair.clone(), &mut order);

			assert_ok!(Marketplace::create_order(
				RuntimeOrigin::signed(seller.clone()),
				order,
				Execution::AllowCreation
			));

			Balances::set_balance(&buyer, 1000000);

			let payout_address = Marketplace::payout_address().unwrap();
			let payout_address_balance_before = Balances::balance(&payout_address);
			let seller_balance_before = Balances::balance(&seller);
			let buyer_balance_before = Balances::balance(&buyer);

			let bid_fee = 0;
			let mut bid = Order {
				order_type: OrderType::Bid,
				collection: 0,
				item: 0,
				expires_at,
				price: price.clone(),
				fee_percent: bid_fee,
				signature_data: SignatureData {
					signature: raw_signature([0; 65]),
					nonce: <Vec<u8>>::new(),
				},
			};
			append_valid_signature(fee_signer_pair, &mut bid);

			assert_ok!(Marketplace::create_order(
				RuntimeOrigin::signed(buyer.clone()),
				bid.clone(),
				Execution::AllowCreation
			));

			let buyer_fee = (price.clone() * bid_fee) / max_basis_points.clone();
			let seller_fee = (price.clone() * ask_fee) / max_basis_points.clone();
			let buyer_payment = price + buyer_fee.clone();
			let marketplace_pay = buyer_fee + seller_fee;
			let seller_pay = buyer_payment.clone() - marketplace_pay.clone();

			assert_eq!(Nfts::owner(0, 0), Some(buyer.clone()));
			assert!(Nfts::can_transfer(&0, &0));
			assert_eq!(
				payout_address_balance_before + marketplace_pay,
				Balances::balance(&payout_address)
			);
			assert_eq!(seller_balance_before + seller_pay, Balances::balance(&seller));
			assert_eq!(buyer_balance_before - buyer_payment, Balances::balance(&buyer))
		})
	}
}

mod cancel_ask {
	use super::*;

	#[test]
	fn ask_not_found() {
		new_test_ext().execute_with(|| {
			assert_noop!(
				Marketplace::cancel_order(
					RuntimeOrigin::signed(account(1)),
					OrderType::Ask,
					0,
					0,
					0
				),
				Error::<Test>::OrderNotFound
			);
		})
	}

	#[test]
	fn not_creator_or_admin() {
		new_test_ext().execute_with(|| {
			let _ = admin_accounts_setup();

			create_valid_order(OrderType::Ask, account(1), account(1));

			assert_noop!(
				Marketplace::cancel_order(
					RuntimeOrigin::signed(account(2)),
					OrderType::Ask,
					0,
					0,
					0
				),
				Error::<Test>::NotOrderCreatorOrAdmin
			);

			assert_ok!(Marketplace::force_set_authority(RuntimeOrigin::root(), account(3)));
			assert_noop!(
				Marketplace::cancel_order(
					RuntimeOrigin::signed(account(2)),
					OrderType::Ask,
					0,
					0,
					0
				),
				Error::<Test>::NotOrderCreatorOrAdmin
			);
		})
	}

	#[test]
	fn cancel_as_seller() {
		new_test_ext().execute_with(|| {
			let _ = admin_accounts_setup();

			create_valid_order(OrderType::Ask, account(1), account(1));

			assert_ok!(Marketplace::cancel_order(
				RuntimeOrigin::signed(account(1)),
				OrderType::Ask,
				0,
				0,
				0
			));

			assert!(Marketplace::asks(0, 0) == None);
			assert!(Nfts::can_transfer(&0, &0));
		})
	}

	#[test]
	fn cancel_as_admin() {
		new_test_ext().execute_with(|| {
			let _ = admin_accounts_setup();

			create_valid_order(OrderType::Ask, account(1), account(1));
			assert_ok!(Marketplace::force_set_authority(RuntimeOrigin::root(), account(3)));

			assert_ok!(Marketplace::cancel_order(
				RuntimeOrigin::signed(account(3)),
				OrderType::Ask,
				0,
				0,
				0
			));

			assert!(Marketplace::asks(0, 0) == None);
			assert!(Nfts::can_transfer(&0, &0));
		})
	}
}
mod cancel_bid {
	use super::*;

	#[test]
	fn bid_not_found() {
		new_test_ext().execute_with(|| {
			assert_noop!(
				Marketplace::cancel_order(
					RuntimeOrigin::signed(account(1)),
					OrderType::Bid,
					0,
					0,
					5
				),
				Error::<Test>::OrderNotFound
			);
		})
	}

	#[test]
	fn not_creator_or_admin() {
		new_test_ext().execute_with(|| {
			let _ = admin_accounts_setup();

			create_valid_order(OrderType::Bid, account(2), account(1));

			assert_noop!(
				Marketplace::cancel_order(
					RuntimeOrigin::signed(account(3)),
					OrderType::Bid,
					0,
					0,
					10000
				),
				Error::<Test>::NotOrderCreatorOrAdmin
			);

			assert_ok!(Marketplace::force_set_authority(RuntimeOrigin::root(), account(4)));
			assert_noop!(
				Marketplace::cancel_order(
					RuntimeOrigin::signed(account(3)),
					OrderType::Bid,
					0,
					0,
					10000
				),
				Error::<Test>::NotOrderCreatorOrAdmin
			);
		})
	}

	#[test]
	fn cancel_as_seller() {
		new_test_ext().execute_with(|| {
			let _ = admin_accounts_setup();

			create_valid_order(OrderType::Bid, account(1), account(2));
			let reserved =
				Balances::balance_on_hold(&HoldReason::MarketplaceBid.into(), &account(1));

			let price = 10000;

			assert_ok!(Marketplace::cancel_order(
				RuntimeOrigin::signed(account(1)),
				OrderType::Bid,
				0,
				0,
				price
			));

			let fee = <Test as Config>::MaxBasisPoints::get();
			assert!(Marketplace::asks(0, 0) == None);

			let bid_payment = Marketplace::calc_bid_payment(&price, &fee).unwrap_or_default();
			assert!(
				bid_payment.saturating_add(Balances::balance_on_hold(
					&HoldReason::MarketplaceBid.into(),
					&account(1)
				)) == reserved
			);
		})
	}

	#[test]
	fn cancel_as_admin() {
		new_test_ext().execute_with(|| {
			let _ = admin_accounts_setup();

			create_valid_order(OrderType::Bid, account(1), account(2));
			assert_ok!(Marketplace::force_set_authority(RuntimeOrigin::root(), account(3)));
			let reserved =
				Balances::balance_on_hold(&HoldReason::MarketplaceBid.into(), &account(1));

			let price = 10000;
			assert_ok!(Marketplace::cancel_order(
				RuntimeOrigin::signed(account(3)),
				OrderType::Bid,
				0,
				0,
				price
			));

			let fee = <Test as Config>::MaxBasisPoints::get();
			assert!(Marketplace::asks(0, 0) == None);

			let bid_payment = Marketplace::calc_bid_payment(&price, &fee).unwrap_or_default();
			assert!(
				bid_payment.saturating_add(Balances::balance_on_hold(
					&HoldReason::MarketplaceBid.into(),
					&account(1)
				)) == reserved
			);
		})
	}
}
