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
use pallet_dmarket::DmarketCollection;
use pallet_dmarket::Pallet as Dmarket;
use pallet_nfts::{
	CollectionConfig, CollectionSettings, ItemConfig, ItemId, MintSettings, Pallet as Nfts,
};
const SEED: u32 = 0;

use crate::BenchmarkHelper;

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

fn mint_nft<T: Config>(nft_id: ItemId) -> T::AccountId {
	let caller: T::AccountId = funded_and_whitelisted_account::<T>("tokenOwner", 0);

	let default_config = CollectionConfig {
		settings: CollectionSettings::all_enabled(),
		max_supply: Some(u128::MAX),
		mint_settings: MintSettings::default(),
	};

	assert_ok!(Nfts::<T>::create_collection(&caller, &caller, &default_config));
	let collection = <T as pallet::Config>::BenchmarkHelper::collection(0);
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
	fn set_item_owner() {
		let migrator: T::AccountId = get_migrator::<T>();
		let collection: T::CollectionId = <T as pallet::Config>::BenchmarkHelper::collection(0);
		let item: ItemId = 1;
		let _ = mint_nft::<T>(item);
		let receiver: T::AccountId = account("receiver", 0, SEED);

		assert_ok!(Dmarket::<T>::force_set_collection(RawOrigin::Root.into(), collection));
		assert_eq!(DmarketCollection::<T>::get().unwrap(), collection);

		#[extrinsic_call]
		_(RawOrigin::Signed(migrator), item, receiver.clone());

		assert_eq!(Nfts::<T>::owner(collection, item), Some(receiver));
	}

	impl_benchmark_test_suite!(Migration, crate::mock::new_test_ext(), crate::mock::Test);
}
