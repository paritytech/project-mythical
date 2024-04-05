#![cfg(feature = "runtime-benchmarks")]
use super::*;
use crate::Pallet as Migration;
use frame_benchmarking::v2::*;
use frame_support::{assert_ok, dispatch::RawOrigin};

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
		let next_collection_id: <T as pallet_nfts::Config>::CollectionId = (1 as u32).into();
		let migrator: T::AccountId = get_migrator::<T>();

		#[extrinsic_call]
		_(RawOrigin::Signed(migrator), next_collection_id.clone());

		assert_last_event::<T>(Event::NextCollectionIdUpdated(next_collection_id).into());
	}

	impl_benchmark_test_suite!(Migration, crate::mock::new_test_ext(), crate::mock::Test);
}
