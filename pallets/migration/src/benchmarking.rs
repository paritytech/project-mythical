#![cfg(feature = "runtime-benchmarks")]
use super::*;
use crate::Pallet as Migration;
use frame_benchmarking::v2::*;
use frame_support::{
	assert_ok,
	traits::{
		fungible::{Inspect as InspectFungible, Mutate as MutateFungible},
		tokens::nonfungibles_v2::{Create, Mutate},
	},
};
use frame_system::RawOrigin;
use pallet_marketplace::Ask;
use pallet_marketplace::BenchmarkHelper;
use pallet_nfts::{
	CollectionConfig, CollectionSettings, ItemConfig, ItemId, MintSettings, Pallet as Nfts,
};
use sp_core::Get;
use sp_runtime::traits::StaticLookup;
const SEED: u32 = 0;

fn assert_last_event<T: Config>(generic_event: <T as Config>::RuntimeEvent) {
	frame_system::Pallet::<T>::assert_last_event(generic_event.into());
}

fn get_migrator<T: Config>() -> T::AccountId {
	let migrator: T::AccountId = funded_and_whitelisted_account::<T>("migrator", 10);
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

fn create_collection<T: Config>() -> (T::CollectionId, T::AccountId) {
	let migrator: T::AccountId = get_migrator::<T>();
	let migrator_lookup = T::Lookup::unlookup(migrator.clone());

	let default_config = CollectionConfig {
		settings: CollectionSettings::all_enabled(),
		max_supply: Some(u128::MAX),
		mint_settings: MintSettings::default(),
	};
	let collection = T::BenchmarkHelper::collection(0);

	assert_ok!(Nfts::<T>::force_create(
		RawOrigin::Signed(migrator).into(),
		migrator_lookup.clone(),
		default_config
	));
	(collection, migrator)
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
#[benchmarks()]
pub mod benchmarks {
	use pallet_nfts::Collection;
	use sp_runtime::BoundedVec;

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
	fn force_create() {
		let migrator: T::AccountId = get_migrator::<T>();
		let migrator_lookup = T::Lookup::unlookup(migrator.clone());

		let default_config = CollectionConfig {
			settings: CollectionSettings::all_enabled(),
			max_supply: Some(u128::MAX),
			mint_settings: MintSettings::default(),
		};

		#[extrinsic_call]
		_(RawOrigin::Signed(migrator), migrator_lookup, default_config);

		assert!(Collection::<T>::get(T::BenchmarkHelper::collection(0)).is_some());
	}

	#[benchmark]
	fn set_collection_metadata() {
		let (collection, migrator) = create_collection::<T>();
		let data: BoundedVec<_, _> = vec![0u8; T::StringLimit::get() as usize].try_into().unwrap();

		#[extrinsic_call]
		_(RawOrigin::Signed(migrator), collection, data);
	}

	#[benchmark]
	fn set_team() {
		let (collection, migrator) = create_collection::<T>();
		let admin = account("admin", 0, SEED);

		#[extrinsic_call]
		_(RawOrigin::Signed(migrator), collection, admin, admin, admin);
	}

	#[benchmark]
	fn force_mint() {
		todo!();

		#[extrinsic_call]
		_(RawOrigin::Signed(migrator), next_collection_id.clone());
	}

	#[benchmark]
	fn create_ask() {
		let migrator: T::AccountId = get_migrator::<T>();
		// Nft Setup
		let collection = T::BenchmarkHelper::collection(0);
		let item = T::BenchmarkHelper::item(1);
		let caller = mint_nft::<T>(item);
		let ask = Ask {
			seller: caller.clone(),
			price: (1000 as u32).into(),
			expiration: T::BenchmarkHelper::timestamp(100),
			fee: (100 as u32).into(),
			escrow_agent: None,
		};

		#[extrinsic_call]
		_(RawOrigin::Signed(migrator), collection.clone(), item.clone(), ask.clone());

		assert_last_event::<T>(Event::AskCreated { collection, item, ask }.into());
	}

	#[benchmark]
	fn send_funds_from_pot() {
		let migrator: T::AccountId = get_migrator::<T>();
		let pot: T::AccountId = Migration::<T>::pot_account_id();
		let receiver: T::AccountId = account("receiver", 0, SEED);
		let ed = <T as Config>::Currency::minimum_balance();
		let pot_multi = BalanceOf::<T>::from(1000u32);
		let send_multi = BalanceOf::<T>::from(10u32);
		let amount_to_send = ed * send_multi;
		<T as Config>::Currency::set_balance(&pot, ed * pot_multi);

		#[extrinsic_call]
		_(RawOrigin::Signed(migrator), receiver.clone(), amount_to_send);

		assert_eq!(<T as Config>::Currency::balance(&receiver), amount_to_send);
	}

	#[benchmark]
	fn set_item_owner() {
		let migrator: T::AccountId = get_migrator::<T>();
		let collection = T::BenchmarkHelper::collection(0);
		let item = T::BenchmarkHelper::item(1);
		let _ = mint_nft::<T>(item);
		let receiver: T::AccountId = account("receiver", 0, SEED);

		#[extrinsic_call]
		_(RawOrigin::Signed(migrator), collection.clone(), item.clone(), receiver.clone());

		assert_eq!(Nfts::<T>::owner(collection, item), Some(receiver));
	}

	#[benchmark]
	fn enable_serial_mint() {
		let migrator: T::AccountId = get_migrator::<T>();
		let collection = T::BenchmarkHelper::collection(0);
		let _ = mint_nft::<T>(1);

		#[extrinsic_call]
		_(RawOrigin::Signed(migrator), collection.clone(), true);
	}

	impl_benchmark_test_suite!(Migration, crate::mock::new_test_ext(), crate::mock::Test);
}
