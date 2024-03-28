use crate::{mock::*, *};

use sp_runtime::{traits::IdentifyAccount, BoundedVec};
type AccountIdOf<Test> = <Test as frame_system::Config>::AccountId;

mod multibatching_test {
	use account::{AccountId20, EthereumSignature, EthereumSigner};
	use frame_support::{assert_noop, assert_ok};
	use parity_scale_codec::Encode;
	use sp_core::{blake2_256, ecdsa::Pair as EthereumPair, keccak_256, Pair};

	use super::*;

	fn account(id: u8) -> AccountIdOf<Test> {
		[id; 20].into()
	}

	#[test]
	fn multibatching_should_work() {
		new_test_ext().execute_with(|| {
			let call_count = 10;
			let signer_count = 10;

			let domain: [u8; 32] = *b".myth.pallet-multibatching.bench";
			let bias = [0u8; 32];

			let sender = account(0);

			let mut signers =
				Vec::<(EthereumPair, EthereumSigner, AccountId20)>::with_capacity(signer_count);
			for _ in 0..signer_count {
				let pair: EthereumPair = EthereumPair::generate().0;
				let signer: EthereumSigner = pair.public().into();
				let account = signer.clone().into_account();
				signers.push((pair, signer, account));
			}

			let mut calls = BoundedVec::new();
			let iter = (0..call_count).into_iter().zip(signers.iter().cycle());
			for (_, (_, signer, _)) in iter {
				let call = frame_system::Call::remark { remark: vec![] }.into();
				calls
					.try_push(BatchedCall::<Test> { from: signer.clone().into(), call })
					.ok()
					.expect("Mock config must match runtime config for BoundedVec size");
			}

			let pseudo_call: <Test as Config>::RuntimeCall = Call::<Test>::batch {
				domain,
				sender: sender.into(),
				bias,
				calls: calls.clone(),
				approvals: BoundedVec::new(),
			}
			.into();
			let pseudo_call_bytes = pseudo_call.encode();
			let hash = keccak_256(&pseudo_call_bytes);

			/* eprintln!("test   bytes: {}", hex::encode(&pseudo_call_bytes));
			eprintln!("test   hash: {}", hex::encode(hash.into())); */

			let mut approvals = BoundedVec::new();
			for (pair, _, account) in &signers {
				approvals
					.try_push(Approval::<Test> {
						from: EthereumSigner::from(account.0).into(),
						signature: EthereumSignature::from(pair.sign_prehashed(&hash.into()))
							.into(),
					})
					.ok()
					.expect("Benchmark config must match runtime config for BoundedVec size");
				eprintln!("test  from: {:?}", &approvals.last().unwrap().from);
				eprintln!("test   sig: {:?}", &approvals.last().unwrap().signature);
			}

			assert_ok!(Multibatching::force_set_domain(RuntimeOrigin::root(), domain));
			assert_ok!(Multibatching::batch(
				RuntimeOrigin::signed(sender.clone()),
				domain,
				sender.clone().into(),
				bias,
				calls,
				approvals,
			));
		})
	}
	#[test]
	fn multibatching_fails_with_wrong_hashing() {
		new_test_ext().execute_with(|| {
			let call_count = 10;
			let signer_count = 10;

			let domain: [u8; 32] = *b".myth.pallet-multibatching.bench";
			let bias = [0u8; 32];

			let sender = account(0);

			let mut signers =
				Vec::<(EthereumPair, EthereumSigner, AccountId20)>::with_capacity(signer_count);
			for _ in 0..signer_count {
				let pair: EthereumPair = EthereumPair::generate().0;
				let signer: EthereumSigner = pair.public().into();
				let account = signer.clone().into_account();
				signers.push((pair, signer, account));
			}

			let mut calls = BoundedVec::new();
			let iter = (0..call_count).into_iter().zip(signers.iter().cycle());
			for (_, (_, signer, _)) in iter {
				let call = frame_system::Call::remark { remark: vec![] }.into();
				calls
					.try_push(BatchedCall::<Test> { from: signer.clone().into(), call })
					.ok()
					.expect("Mock config must match runtime config for BoundedVec size");
			}

			let pseudo_call: <Test as Config>::RuntimeCall = Call::<Test>::batch {
				domain,
				sender: sender.into(),
				bias,
				calls: calls.clone(),
				approvals: BoundedVec::new(),
			}
			.into();
			let pseudo_call_bytes = pseudo_call.encode();

			// An ethereum signature must be hashed with keccak_256 so using other hashing 
			// methods will result in invalid signature
			let hash = blake2_256(&pseudo_call_bytes);

			/* eprintln!("test   bytes: {}", hex::encode(&pseudo_call_bytes));
			eprintln!("test   hash: {}", hex::encode(hash.into())); */

			let mut approvals = BoundedVec::new();
			for (pair, _, account) in &signers {
				approvals
					.try_push(Approval::<Test> {
						from: EthereumSigner::from(account.0).into(),
						signature: EthereumSignature::from(pair.sign_prehashed(&hash.into()))
							.into(),
					})
					.ok()
					.expect("Benchmark config must match runtime config for BoundedVec size");
				eprintln!("test  from: {:?}", &approvals.last().unwrap().from);
				eprintln!("test   sig: {:?}", &approvals.last().unwrap().signature);
			}

			assert_ok!(Multibatching::force_set_domain(RuntimeOrigin::root(), domain));
			assert_noop!(Multibatching::batch(
				RuntimeOrigin::signed(sender.clone()),
				domain,
				sender.clone().into(),
				bias,
				calls,
				approvals,
			), Error::<Test>::InvalidSignature(0));
		})
	}
}
