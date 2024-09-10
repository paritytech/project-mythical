#![cfg(feature = "runtime-benchmarks")]
use super::*;
use crate::Pallet as Marketplace;
use frame_benchmarking::v2::*;
use frame_support::{
	assert_ok,
	dispatch::RawOrigin,
	traits::{
		fungible::{Inspect as InspectFungible, Mutate as MutateFungible},
		tokens::nonfungibles_v2::{Create, Mutate},
	},
};
use pallet_nfts::ItemId;
use pallet_nfts::{CollectionConfig, CollectionSettings, ItemConfig, MintSettings, Pallet as Nfts};
use sp_core::ecdsa::Public;
use sp_io::{
	crypto::{ecdsa_generate, ecdsa_sign_prehashed},
	hashing::keccak_256,
};
use sp_std::vec;

use sp_core::ecdsa::Signature;

const SEED: u32 = 0;

type BalanceOf<T> =
	<<T as Config>::Currency as InspectFungible<<T as frame_system::Config>::AccountId>>::Balance;

impl<CollectionId, ItemId, Moment> BenchmarkHelper<CollectionId, ItemId, Moment> for ()
where
	CollectionId: From<u16>,
	ItemId: From<u16>,
	Moment: From<u64>,
{
	fn collection(id: u16) -> CollectionId {
		id.into()
	}
	fn item(id: u16) -> ItemId {
		id.into()
	}
	fn timestamp(value: u64) -> Moment {
		value.into()
	}
}

fn assert_last_event<T: Config>(generic_event: <T as Config>::RuntimeEvent) {
	frame_system::Pallet::<T>::assert_last_event(generic_event.into());
}

fn funded_and_whitelisted_account<T: Config>(name: &'static str, index: u32) -> T::AccountId {
	let caller: T::AccountId = account(name, index, SEED);
	// Give the account half of the maximum value of the `Balance` type.
	// Otherwise some transfers will fail with an overflow error.
	let ed = <T as Config>::Currency::minimum_balance();
	let multiplier = BalanceOf::<T>::from(1000000u32);

	<T as Config>::Currency::set_balance(&caller, ed * multiplier);
	whitelist_account!(caller);
	caller
}

fn get_admin<T: Config>() -> T::AccountId {
	let admin: T::AccountId = account("admin", 10, SEED);
	whitelist_account!(admin);
	assert_ok!(Marketplace::<T>::force_set_authority(RawOrigin::Root.into(), admin.clone()));

	admin
}

fn mint_nft<T: Config>(nft_id: ItemId) -> T::AccountId {
	let caller: T::AccountId = funded_and_whitelisted_account::<T>("tokenOwner", 0);

	let default_config = CollectionConfig {
		settings: CollectionSettings::all_enabled(),
		max_supply: Some(u128::MAX),
		mint_settings: MintSettings::default(),
	};

	assert_ok!(Nfts::<T>::create_collection(&caller, &caller, &default_config));
	let collection = T::BenchmarkHelper::collection(0);
	assert_ok!(Nfts::<T>::mint_into(&collection, &nft_id, &caller, &ItemConfig::default(), true));
	caller
}

#[benchmarks(where T::AccountId: From<AccountId20>, T::Signature: From<EthereumSignature>)]
pub mod benchmarks {
	use super::*;
	use account::{AccountId20, EthereumSignature, EthereumSigner};
	use pallet_timestamp::Pallet as Timestamp;
	use parity_scale_codec::Encode;

	use sp_runtime::traits::IdentifyAccount;

	fn create_valid_order<T: Config>(
		order_type: OrderType,
		caller: T::AccountId,
		price: BalanceOf<T>,
		fee_signer: Public,
		escrow_agent: Option<T::AccountId>,
	) where
		T::Signature: From<EthereumSignature>,
	{
		let mut order = Order {
			order_type,
			collection: T::BenchmarkHelper::collection(0),
			item: T::BenchmarkHelper::item(1),
			expires_at: Timestamp::<T>::get() + T::BenchmarkHelper::timestamp(100000),
			price,
			fee: BalanceOf::<T>::from(0u8),
			escrow_agent,
			signature_data: SignatureData {
				signature: EthereumSignature::from(Signature::from_raw([0; 65])).into(),
				nonce: vec![0],
			},
		};
		append_valid_signature::<T>(fee_signer, &mut order);

		assert_ok!(Marketplace::<T>::create_order(
			RawOrigin::Signed(caller).into(),
			order,
			Execution::AllowCreation
		));
	}

	fn append_valid_signature<T: Config>(fee_signer: Public, order: &mut OrderOf<T>)
	where
		T::Signature: From<EthereumSignature>,
	{
		let message: OrderMessageOf<T> = order.clone().into();

		let hashed = keccak_256(&message.encode());

		let signature =
			EthereumSignature::from(ecdsa_sign_prehashed(0.into(), &fee_signer, &hashed).unwrap());
		order.signature_data.signature = signature.into();
	}

	fn admin_accounts_setup<T: Config>() -> (T::AccountId, Public)
	where
		T::AccountId: From<AccountId20>,
	{
		let admin_public = ecdsa_generate(0.into(), None);
		let admin_signer: EthereumSigner = admin_public.into();
		let admin: T::AccountId = admin_signer.clone().into_account().into();

		let ed = <T as Config>::Currency::minimum_balance();
		let multiplier = BalanceOf::<T>::from(10000u16);
		<T as Config>::Currency::set_balance(&admin, ed * multiplier);
		whitelist_account!(admin);

		assert_ok!(Marketplace::<T>::force_set_authority(RawOrigin::Root.into(), admin.clone()));

		assert_ok!(Marketplace::<T>::set_fee_signer_address(
			RawOrigin::Signed(admin.clone()).into(),
			admin.clone(),
		));
		assert_ok!(Marketplace::<T>::set_payout_address(
			RawOrigin::Signed(admin.clone()).into(),
			admin.clone(),
		));

		(admin, admin_public)
	}

	#[benchmark]
	fn force_set_authority() {
		let authority: T::AccountId = account("authority", 0, SEED);

		#[extrinsic_call]
		_(RawOrigin::Root, authority.clone());

		assert_last_event::<T>(Event::AuthorityUpdated { authority }.into());
	}

	#[benchmark]
	fn set_fee_signer_address() {
		let admin: T::AccountId = get_admin::<T>();
		let fee_signer: T::AccountId = account("feeSigner", 0, SEED);

		#[extrinsic_call]
		_(RawOrigin::Signed(admin), fee_signer.clone());

		assert_last_event::<T>(Event::FeeSignerAddressUpdate { fee_signer }.into());
	}

	#[benchmark]
	fn set_payout_address() {
		let admin: T::AccountId = get_admin::<T>();
		let payout_address: T::AccountId = account("payoutAddress", 0, SEED);

		#[extrinsic_call]
		_(RawOrigin::Signed(admin), payout_address.clone());

		assert_last_event::<T>(Event::PayoutAddressUpdated { payout_address }.into());
	}

	// Benchmark `create_order` wxtrinsic with the worst possible conditions:
	// Ask already exists
	// Matching Bid is created and executed
	#[benchmark]
	fn create_order() {
		// Nft setup
		let item = T::BenchmarkHelper::item(1);
		let seller = mint_nft::<T>(item);
		// Create ask order
		let (_, fee_signer) = admin_accounts_setup::<T>();

		let ed = <T as Config>::Currency::minimum_balance();
		let price = ed * BalanceOf::<T>::from(100u16);
		let escrow: T::AccountId = funded_and_whitelisted_account::<T>("escrow", 0);

		create_valid_order::<T>(
			OrderType::Ask,
			seller.clone(),
			price,
			fee_signer,
			Some(escrow.clone()),
		);

		// Setup buyer
		let buyer: T::AccountId = funded_and_whitelisted_account::<T>("buyer", 0);
		let mut order = Order {
			order_type: OrderType::Bid,
			collection: T::BenchmarkHelper::collection(0),
			item,
			expires_at: Timestamp::<T>::get() + T::BenchmarkHelper::timestamp(100000),
			price,
			fee: ed,
			escrow_agent: Some(escrow),
			signature_data: SignatureData {
				signature: EthereumSignature::from(Signature::from_raw([0; 65])).into(),
				nonce: vec![1],
			},
		};
		append_valid_signature::<T>(fee_signer, &mut order);

		#[extrinsic_call]
		_(RawOrigin::Signed(buyer.clone()), order.clone(), Execution::AllowCreation);

		assert_last_event::<T>(
			Event::OrderExecuted {
				collection: order.collection,
				item: order.item,
				seller,
				buyer,
				price: order.price,
				seller_fee: BalanceOf::<T>::from(0u8),
				buyer_fee: order.fee,
			}
			.into(),
		);
	}

	// Benchmark `cancel_order` extrinsic with the worst possible conditions:
	// Cancel a bid
	#[benchmark]
	fn cancel_order() {
		// Nft Setup
		let collection = T::BenchmarkHelper::collection(0);
		let item = T::BenchmarkHelper::item(1);
		let _ = mint_nft::<T>(item);

		// Setup Bid order
		let price = BalanceOf::<T>::from(10000u16);

		let bidder: T::AccountId = funded_and_whitelisted_account::<T>("bidder", 0);

		let (_, fee_signer_public) = admin_accounts_setup::<T>();
		create_valid_order::<T>(OrderType::Bid, bidder.clone(), price, fee_signer_public, None);

		#[extrinsic_call]
		_(RawOrigin::Signed(bidder.clone()), OrderType::Bid, collection, item, price);

		assert_last_event::<T>(Event::OrderCanceled { collection, item, who: bidder }.into());
	}

	impl_benchmark_test_suite!(Marketplace, crate::mock::new_test_ext(), crate::mock::Test);
}
