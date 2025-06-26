use super::*;
use crate::Pallet as Dmarket;
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

use crate::BenchmarkHelper;

const SEED: u32 = 0;

type BalanceOf<T> =
	<<T as Config>::Currency as InspectFungible<<T as frame_system::Config>::AccountId>>::Balance;

impl<CollectionId, Moment> BenchmarkHelper<CollectionId, Moment> for ()
where
	CollectionId: From<u16>,
	ItemId: From<u16>,
	Moment: From<u64>,
{
	fn collection(id: u16) -> CollectionId {
		id.into()
	}
	fn timestamp(value: u64) -> Moment {
		value.into()
	}
}

fn assert_last_event<T: Config>(generic_event: <T as frame_system::Config>::RuntimeEvent) {
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

fn mint_nft<T: Config>(nft_id: ItemId, caller: T::AccountId) {
	let default_config = CollectionConfig {
		settings: CollectionSettings::all_enabled(),
		max_supply: Some(u128::MAX),
		mint_settings: MintSettings::default(),
	};

	assert_ok!(Nfts::<T>::create_collection(&caller, &caller, &default_config));
	let collection = T::BenchmarkHelper::collection(0);
	assert_ok!(Nfts::<T>::mint_into(&collection, &nft_id, &caller, &ItemConfig::default(), true));
}

#[benchmarks(where T::AccountId: From<AccountId20>, T::Signature: From<EthereumSignature>)]
pub mod benchmarks {
	use super::*;
	use account::{AccountId20, EthereumSignature, EthereumSigner};
	use pallet_timestamp::Pallet as Timestamp;

	use sp_runtime::traits::IdentifyAccount;

	fn sign_trade<T: Config>(
		sender: &T::AccountId,
		fee_address: &T::AccountId,
		trade: &TradeParamsOf<T>,
		seller_signer: Public,
		buyer_signer: Public,
	) -> TradeSignatures<T::Signature>
	where
		T::Signature: From<EthereumSignature>,
	{
		let ask_message: Vec<u8> = Dmarket::<T>::get_ask_message(sender, fee_address, trade);
		let ask_hashed = keccak_256(&ask_message);

		let bid_message: Vec<u8> = Dmarket::<T>::get_bid_message(sender, fee_address, trade);
		let bid_hashed = keccak_256(&bid_message);

		TradeSignatures {
			ask_signature: EthereumSignature::from(
				ecdsa_sign_prehashed(0.into(), &seller_signer, &ask_hashed).unwrap(),
			)
			.into(),
			bid_signature: EthereumSignature::from(
				ecdsa_sign_prehashed(1.into(), &buyer_signer, &bid_hashed).unwrap(),
			)
			.into(),
		}
	}

	fn trade_participants<T: Config>() -> (T::AccountId, Public, T::AccountId, Public)
	where
		T::AccountId: From<AccountId20>,
	{
		let ed = <T as Config>::Currency::minimum_balance();
		let multiplier = BalanceOf::<T>::from(10000u16);

		let seller_public = ecdsa_generate(0.into(), None);
		let seller_signer: EthereumSigner = seller_public.into();
		let seller: T::AccountId = seller_signer.clone().into_account().into();
		whitelist_account!(seller);

		let buyer_public = ecdsa_generate(1.into(), None);
		let buyer_signer: EthereumSigner = buyer_public.into();
		let buyer: T::AccountId = buyer_signer.clone().into_account().into();
		whitelist_account!(buyer);

		<T as Config>::Currency::set_balance(&seller, ed * multiplier);
		<T as Config>::Currency::set_balance(&buyer, ed * multiplier);

		(seller, seller_public, buyer, buyer_public)
	}

	#[benchmark]
	fn force_set_collection() {
		let collection_id = T::BenchmarkHelper::collection(0);
		let caller: T::AccountId = funded_and_whitelisted_account::<T>("caller", 0);
		mint_nft::<T>(1, caller);

		#[extrinsic_call]
		_(RawOrigin::Root, collection_id);

		assert_last_event::<T>(Event::CollectionUpdated { collection_id }.into());
	}

	#[benchmark]
	fn execute_trade() {
		let sender: T::AccountId = funded_and_whitelisted_account::<T>("sender", 0);
		let (seller, seller_public, buyer, buyer_public) = trade_participants::<T>();
		let fee_address: T::AccountId = funded_and_whitelisted_account::<T>("fee_address", 0);

		let collection_id = T::BenchmarkHelper::collection(0);
		let item = 1;
		mint_nft::<T>(item, seller.clone());
		assert_ok!(Dmarket::<T>::force_set_collection(RawOrigin::Root.into(), collection_id));

		let expiration = Timestamp::<T>::get() + T::BenchmarkHelper::timestamp(1000);
		let trade = TradeParams {
			price: BalanceOf::<T>::from(100u16),
			fee: BalanceOf::<T>::from(1u8),
			ask_expiration: expiration,
			bid_expiration: expiration,
			item,
		};
		let signatures =
			sign_trade::<T>(&sender, &fee_address, &trade, seller_public, buyer_public);

		#[extrinsic_call]
		_(
			RawOrigin::Signed(sender),
			seller.clone(),
			buyer.clone(),
			trade.clone(),
			signatures,
			fee_address,
		);

		assert_last_event::<T>(
			Event::Trade { seller, buyer, item, price: trade.price, fee: trade.fee }.into(),
		);
	}

	impl_benchmark_test_suite!(Dmarket, crate::mock::new_test_ext(), crate::mock::Test);
}
