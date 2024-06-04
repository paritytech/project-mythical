use crate::*;

use crate::Pallet as Proxy;
use frame_benchmarking::v2::*;
use frame_support::assert_ok;
use frame_system::RawOrigin;
use sp_runtime::Saturating;
use sp_std::vec;

fn assert_last_event<T: Config>(generic_event: <T as Config>::RuntimeEvent) {
	frame_system::Pallet::<T>::assert_last_event(generic_event.into());
}

fn initial_balance<T: pallet::Config>() -> BalanceOf<T> {
	T::Currency::minimum_balance().saturating_mul(10u32.into())
}

#[benchmarks]
mod benchmarks {
	use super::*;

	#[benchmark]
	fn add_proxy() {
		let delegator: <T as frame_system::Config>::AccountId = account("delegator", 0, 0);
		let delegate: <T as frame_system::Config>::AccountId = account("delegate", 0, 0);
		let sponsor: <T as frame_system::Config>::AccountId = account("sponsor", 0, 0);
		let sponsor_agent: <T as frame_system::Config>::AccountId = account("sponsor_agent", 0, 0);
		let proxy_type = T::ProxyType::default();

		assert_ok!(T::Currency::mint_into(&sponsor, initial_balance::<T>()));

		assert_ok!(Proxy::<T>::register_sponsor_agent(
			RawOrigin::Signed(sponsor.clone()).into(),
			sponsor_agent.clone(),
		));

		assert_ok!(Proxy::<T>::approve_proxy_funding(
			RawOrigin::Signed(sponsor_agent.clone()).into(),
			sponsor.clone(),
			delegator.clone(),
		));

		#[extrinsic_call]
		_(
			RawOrigin::Signed(delegator.clone()),
			delegate.clone(),
			proxy_type.clone(),
			Some(sponsor.clone()),
		);

		assert!(Proxy::<T>::has_proxy(&delegator, &delegate));
	}

	#[benchmark]
	fn remove_proxy() {
		let delegator: <T as frame_system::Config>::AccountId = account("delegator", 0, 0);
		let delegate: <T as frame_system::Config>::AccountId = account("delegate", 0, 0);
		let sponsor: <T as frame_system::Config>::AccountId = account("sponsor", 0, 0);
		let sponsor_agent: <T as frame_system::Config>::AccountId = account("sponsor_agent", 0, 0);
		let proxy_type = T::ProxyType::default();

		assert_ok!(T::Currency::mint_into(&sponsor, initial_balance::<T>()));

		assert_ok!(Proxy::<T>::register_sponsor_agent(
			RawOrigin::Signed(sponsor.clone()).into(),
			sponsor_agent.clone(),
		));

		assert_ok!(Proxy::<T>::approve_proxy_funding(
			RawOrigin::Signed(sponsor_agent.clone()).into(),
			sponsor.clone(),
			delegator.clone(),
		));

		assert_ok!(Proxy::<T>::add_proxy(
			RawOrigin::Signed(delegator.clone()).into(),
			delegate.clone(),
			proxy_type.clone(),
			Some(sponsor.clone()),
		));

		#[extrinsic_call]
		_(RawOrigin::Signed(delegator.clone()), delegate.clone());

		assert!(!Proxy::<T>::has_proxy(&delegator, &delegate));
	}

	#[benchmark]
	fn proxy() {
		let delegator: <T as frame_system::Config>::AccountId = account("delegator", 0, 0);
		let delegate: <T as frame_system::Config>::AccountId = account("delegate", 0, 0);
		let proxy_type = T::ProxyType::default();

		assert_ok!(T::Currency::mint_into(&delegator, initial_balance::<T>()));

		assert_ok!(Proxy::<T>::add_proxy(
			RawOrigin::Signed(delegator.clone()).into(),
			delegate.clone(),
			proxy_type.clone(),
			None,
		));

		let call: <T as Config>::RuntimeCall =
			frame_system::Call::<T>::remark { remark: vec![] }.into();

		#[extrinsic_call]
		_(RawOrigin::Signed(delegate.clone()), delegator.clone(), Box::new(call.clone()));

		assert_last_event::<T>(Event::ProxyExecuted { delegate, delegator }.into());
	}

	#[benchmark]
	fn approve_proxy_funding() {
		let delegator: <T as frame_system::Config>::AccountId = account("delegator", 0, 0);
		let sponsor: <T as frame_system::Config>::AccountId = account("sponsor", 0, 0);
		let sponsor_agent: <T as frame_system::Config>::AccountId = account("sponsor_agent", 0, 0);

		assert_ok!(Proxy::<T>::register_sponsor_agent(
			RawOrigin::Signed(sponsor.clone()).into(),
			sponsor_agent.clone()
		));

		#[extrinsic_call]
		_(RawOrigin::Signed(sponsor_agent.clone()), sponsor.clone(), delegator.clone());

		assert!(Proxy::<T>::has_sponsorship_approval(&delegator, &sponsor));
	}

	#[benchmark]
	fn register_sponsor_agent() {
		let sponsor: <T as frame_system::Config>::AccountId = account("sponsor", 0, 0);
		let sponsor_agent: <T as frame_system::Config>::AccountId = account("sponsor_agent", 0, 0);

		#[extrinsic_call]
		_(RawOrigin::Signed(sponsor.clone()), sponsor_agent.clone());

		assert!(Proxy::<T>::has_sponsor_agent(&sponsor, &sponsor_agent));
	}

	#[benchmark]
	fn revoke_sponsor_agent() {
		let sponsor: <T as frame_system::Config>::AccountId = account("sponsor", 0, 0);
		let sponsor_agent: <T as frame_system::Config>::AccountId = account("sponsor_agent", 0, 0);

		assert_ok!(Proxy::<T>::register_sponsor_agent(
			RawOrigin::Signed(sponsor.clone()).into(),
			sponsor_agent.clone()
		));

		#[extrinsic_call]
		_(RawOrigin::Signed(sponsor.clone()), sponsor_agent.clone());

		assert!(!Proxy::<T>::has_sponsor_agent(&sponsor, &sponsor_agent));
	}

	#[benchmark]
	fn remove_sponsored_proxy() {
		let delegator: <T as frame_system::Config>::AccountId = account("delegator", 0, 0);
		let delegate: <T as frame_system::Config>::AccountId = account("delegate", 0, 0);
		let sponsor: <T as frame_system::Config>::AccountId = account("sponsor", 0, 0);
		let sponsor_agent: <T as frame_system::Config>::AccountId = account("sponsor_agent", 0, 0);
		let proxy_type = T::ProxyType::default();

		assert_ok!(T::Currency::mint_into(&sponsor, initial_balance::<T>()));

		assert_ok!(Proxy::<T>::register_sponsor_agent(
			RawOrigin::Signed(sponsor.clone()).into(),
			sponsor_agent.clone(),
		));

		assert_ok!(Proxy::<T>::approve_proxy_funding(
			RawOrigin::Signed(sponsor_agent.clone()).into(),
			sponsor.clone(),
			delegator.clone(),
		));

		assert_ok!(Proxy::<T>::add_proxy(
			RawOrigin::Signed(delegator.clone()).into(),
			delegate.clone(),
			proxy_type.clone(),
			Some(sponsor.clone()),
		));

		#[extrinsic_call]
		_(RawOrigin::Signed(sponsor.clone()), delegator.clone(), delegate.clone());

		assert!(!Proxy::<T>::has_proxy(&delegator, &delegate));
	}

	#[benchmark]
	fn cleanup_approvals() {
		let delegator: <T as frame_system::Config>::AccountId = account("delegator", 0, 0);
		let sponsor: <T as frame_system::Config>::AccountId = account("sponsor", 0, 0);
		let sponsor_agent: <T as frame_system::Config>::AccountId = account("sponsor_agent", 0, 0);

		assert_ok!(Proxy::<T>::register_sponsor_agent(
			RawOrigin::Signed(sponsor.clone()).into(),
			sponsor_agent.clone()
		));

		assert_ok!(Proxy::<T>::approve_proxy_funding(
			RawOrigin::Signed(sponsor_agent.clone()).into(),
			sponsor.clone(),
			delegator.clone(),
		));

		assert_ok!(Proxy::<T>::revoke_sponsor_agent(
			RawOrigin::Signed(sponsor.clone()).into(),
			sponsor_agent.clone()
		));

		#[block]
		{
			Proxy::<T>::cleanup_approvals();
		}

		assert!(!SponsorshipApprovals::<T>::contains_key((delegator, sponsor)));

		assert!(!Proxy::<T>::cleanup_approvals());
	}

	impl_benchmark_test_suite! {
		Proxy,
		crate::mock::new_test_ext(),
		crate::mock::Test,
	}
}
