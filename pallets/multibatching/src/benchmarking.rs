#![cfg(feature = "runtime-benchmarks")]
#![allow(unused_imports)]
use super::*;
use crate::Pallet as Multibatching;
use frame_benchmarking::v2::*;
use frame_support::{
	assert_ok,
    BoundedVec,
	dispatch::RawOrigin,
	traits::{
		fungible::{Inspect as InspectFungible, Mutate as MutateFungible},
		tokens::nonfungibles_v2::{Create, Mutate},
	},
};
use sp_core::ecdsa::Public;
use sp_io::{
	crypto::{ecdsa_generate, ecdsa_sign_prehashed},
	hashing::keccak_256,
};

use sp_core::ecdsa::Signature;

#[benchmarks(
    where
        T::Signer: From<EthereumSigner>,
        T::Signature: From<EthereumSignature>,
        T::Hash: From<[u8;32]>,
        T::Hash: Into<[u8;32]>,
        <T as Config>::RuntimeCall: From<Call<T>>,
        <T as frame_system::Config>::AccountId: From<AccountId20>,
        <T as frame_system::Config>::RuntimeEvent: From<Event<T>>,
        <T as frame_system::Config>::RuntimeOrigin: From<frame_system::RawOrigin<AccountId20>>,
)]
pub mod benchmarks {
	use super::*;
	use account::{AccountId20, EthereumSignature, EthereumSigner};
	use parity_scale_codec::Encode;

	use frame_support::sp_runtime::traits::{IdentifyAccount, Hash};

    #[benchmark]
    fn batch(c: Linear<1, 200>, s: Linear<1, 10>) {
        let call_count = c as usize;
        let signer_count = s as usize;

        let domain: [u8; 32] = *b".myth.pallet-multibatching.bench";
        let bias = [0u8; 32];

        let sender: AccountId20 = whitelisted_caller();

        let mut signers = Vec::<(Public, EthereumSigner, AccountId20)>::with_capacity(signer_count);
        for _ in 0..signer_count {
            let public = ecdsa_generate(0.into(), None);
            let signer: EthereumSigner = public.into();
            let account = signer.clone().into_account().into();
            signers.push((public, signer, account));
        }

        let mut calls = BoundedVec::new();
        let iter = (0..call_count).into_iter().zip(signers.iter().cycle());
        for (_, (_, signer, _)) in iter  {
            let call = frame_system::Call::remark { remark: vec![] }.into();
            calls.try_push(BatchedCall::<T>{ from: signer.clone().into(), call })
                .ok().expect("Benchmark config must match runtime config for BoundedVec size");
        }

        let pseudo_call: <T as Config>::RuntimeCall = Call::<T>::batch{
            domain,
            sender: sender.into(),
            bias,
            calls: calls.clone(),
            approvals: BoundedVec::new(),
        }.into();
        let pseudo_call_bytes = pseudo_call.encode();
        let hash = <T::Hashing>::hash(&pseudo_call_bytes);

        eprintln!("test   bytes: {}", hex::encode(&pseudo_call_bytes));
        eprintln!("test   hash: {}", hex::encode(hash.into()));

        let mut approvals = BoundedVec::new();
        for (public, _, account) in &signers {
            approvals.try_push(Approval::<T>{
                from: EthereumSigner::from(account.0).into(),
                signature: EthereumSignature::from(ecdsa_sign_prehashed(0.into(), public, &hash.into()).unwrap()).into(),
            }).ok().expect("Benchmark config must match runtime config for BoundedVec size");
            eprintln!("test  from: {:?}", &approvals.last().unwrap().from);
            eprintln!("test   sig: {:?}", &approvals.last().unwrap().signature);
        }

        Pallet::<T>::force_set_domain(RawOrigin::Root.into(), domain).expect("force_set_domain must succeed");

        #[extrinsic_call]
        _(
            RawOrigin::Signed(sender.clone()),
            domain,
            sender.clone().into(),
            bias,
            calls,
            approvals,
        );
        
        frame_system::Pallet::<T>::assert_last_event(Event::BatchApplied{ hash }.into());
    }

    impl_benchmark_test_suite!(Multibatching, crate::mock::new_test_ext(), crate::mock::Test);
}

