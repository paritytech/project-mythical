#![cfg(feature = "runtime-benchmarks")]
use super::*;
use crate::Pallet as Migration;
use frame_benchmarking::v2::*;
use frame_support::{
	assert_ok,
	dispatch::RawOrigin,
	traits::{
		fungible::{Inspect as InspectFungible, Mutate as MutateFungible},
		tokens::nonfungibles_v2::{Create, Mutate},
	},
};
use pallet_marketplace::Ask;
use pallet_marketplace::BenchmarkHelper;
use pallet_nfts::{CollectionConfig, CollectionSettings, ItemConfig, MintSettings, Pallet as Nfts};
const SEED: u32 = 0;

fn assert_last_event<T: Config>(generic_event: <T as Config>::RuntimeEvent) {
	frame_system::Pallet::<T>::assert_last_event(generic_event.into());
}

fn get_migrator<T: Config>() -> T::AccountId {
	let migrator: T::AccountId = account("migrator", 10, SEED);
	whitelist_account!(migrator);
	assert_ok!(Migration::<T>::force_set_migrator(RawOrigin::Root.into(), migrator.clone()));

	migrator
}

fn funded_and_whitelisted_account<T: Config>(name: &'static str, index: u32) -> T::AccountId {
	let caller: T::AccountId = account(name, index, SEED);
	// Give the account half of the maximum value of the `Balance` type.
	let ed = <T as Config>::Currency::minimum_balance();
	let multiplier = BalanceOf::<T>::from(1000000u32);

	<T as Config>::Currency::set_balance(&caller, ed * multiplier);
	whitelist_account!(caller);
	caller
}

fn mint_nft<T: Config>(nft_id: T::ItemId) -> T::AccountId {
	let caller: T::AccountId = funded_and_whitelisted_account::<T>("tokenOwner", 0);

	let default_config = CollectionConfig {
		settings: CollectionSettings::all_enabled(),
		max_supply: None,
		mint_settings: MintSettings::default(),
	};

	assert_ok!(Nfts::<T>::create_collection(&caller, &caller, &default_config));
	let collection = T::BenchmarkHelper::collection(0);
	assert_ok!(Nfts::<T>::mint_into(&collection, &nft_id, &caller, &ItemConfig::default(), true));
	caller
}
#[benchmarks()]
pub mod benchmarks {
	use super::*;

	#[benchmark]
	fn force_set_migrator() {
		let migrator: T::AccountId = account("migrator", 0, SEED);

		#[extrinsic_call]
		_(RawOrigin::Root, migrator.clone());

		assert_last_event::<T>(Event::MigratorUpdated(migrator).into());
	}

	#[benchmark]
	fn set_next_collection_id() {
		let next_collection_id = T::BenchmarkHelper::collection(0);
		let migrator: T::AccountId = get_migrator::<T>();

		#[extrinsic_call]
		_(RawOrigin::Signed(migrator), next_collection_id.clone());

		assert_last_event::<T>(Event::NextCollectionIdUpdated(next_collection_id).into());
	}

	#[benchmark]
	fn create_ask() {
		let migrator: T::AccountId = get_migrator::<T>();
		// Nft Setup
		let collection = T::BenchmarkHelper::collection(0);
		let item = T::BenchmarkHelper::item(0);
		let caller = mint_nft::<T>(item);
		let ask = Ask {
			seller: caller.clone(),
			price: (1000 as u32).into(),
			expiration: T::BenchmarkHelper::timestamp(100),
			fee: (100 as u32).into(),
		};

		#[extrinsic_call]
		_(RawOrigin::Signed(migrator), collection.clone(), item.clone(), ask.clone());

		assert_last_event::<T>(Event::AskCreated { collection, item, ask }.into());
	}

	#[benchmark]
	fn set_pot_account() {
		let migrator: T::AccountId = get_migrator::<T>();
		let pot: T::AccountId = account("pot", 0, SEED);

		#[extrinsic_call]
		_(RawOrigin::Signed(migrator), pot.clone());

		assert_last_event::<T>(Event::PotUpdated(pot).into());
	}

	#[benchmark]
	fn send_funds_from_pot() {
		let migrator: T::AccountId = get_migrator::<T>();
		let pot: T::AccountId = account("pot", 0, SEED);
		let receiver: T::AccountId = account("receiver", 0, SEED);
		let ed = <T as Config>::Currency::minimum_balance();
		let pot_multi = BalanceOf::<T>::from(1000u32);
		let send_multi = BalanceOf::<T>::from(10u32);
		let amount_to_send = ed * send_multi;
		<T as Config>::Currency::set_balance(&pot, ed * pot_multi);
		assert_ok!(Migration::<T>::set_pot_account(
			RawOrigin::Signed(migrator.clone()).into(),
			pot.clone()
		));

		#[extrinsic_call]
		_(RawOrigin::Signed(migrator), receiver.clone(), amount_to_send);

		assert_eq!(<T as Config>::Currency::balance(&receiver), amount_to_send);
	}

	#[benchmark]
	fn set_item_owner() {
		let migrator: T::AccountId = get_migrator::<T>();
		let collection = T::BenchmarkHelper::collection(0);
		let item = T::BenchmarkHelper::item(0);
		let _ = mint_nft::<T>(item);
		let receiver: T::AccountId = account("receiver", 0, SEED);

		#[extrinsic_call]
		_(RawOrigin::Signed(migrator), collection.clone(), item.clone(), receiver.clone());

		assert_eq!(Nfts::<T>::owner(collection, item), Some(receiver));
	}

	impl_benchmark_test_suite!(Migration, crate::mock::new_test_ext(), crate::mock::Test);
}
