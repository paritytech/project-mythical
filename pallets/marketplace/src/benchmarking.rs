#![cfg(feature = "runtime-benchmarks")]

use super::*;
use crate::Pallet as Marketplace;

use frame_benchmarking::v2::*;
use frame_support::{assert_ok, dispatch::RawOrigin};

const SEED: u32 = 0;

fn assert_last_event<T: Config>(generic_event: <T as Config>::RuntimeEvent) {
	frame_system::Pallet::<T>::assert_last_event(generic_event.into());
}
fn get_admin<T: Config>() -> T::AccountId {
	let admin: T::AccountId = account("admin", 10, SEED);
	assert_ok!(Marketplace::<T>::force_set_authority(RawOrigin::Root.into(), admin.clone()));

	admin
}
#[benchmarks()]
pub mod benchmarks {
	use super::*;

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

	impl_benchmark_test_suite!(Marketplace, crate::mock::new_test_ext(), crate::mock::Test);
}
