#![cfg(feature = "runtime-benchmarks")]
use super::*;
use crate::Pallet as Migration;
use frame_benchmarking::v2::*;
use frame_support::{assert_ok, dispatch::RawOrigin};

const SEED: u32 = 0;

fn assert_last_event<T: Config>(generic_event: <T as Config>::RuntimeEvent) {
	frame_system::Pallet::<T>::assert_last_event(generic_event.into());
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

    impl_benchmark_test_suite!(Marketplace, crate::mock::new_test_ext(), crate::mock::Test);
}
