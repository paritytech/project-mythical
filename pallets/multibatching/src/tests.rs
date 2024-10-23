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

			let domain: [u8; 8] = *b"MYTH_NET";
			let bias = [0u8; 32];
			let expires_at = Timestamp::get().saturating_add(
				<Test as pallet_timestamp::Config>::Moment::from(1_000_000_000_u64),
			);

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
				expires_at,
				calls: calls.clone(),
				approvals: BoundedVec::new(),
			}
			.into();
			let pseudo_call_bytes = pseudo_call.encode();
			let hash = keccak_256(&pseudo_call_bytes);

			//eprintln!("test   bytes: {}", hex::encode(&pseudo_call_bytes));
			//eprintln!("test   hash: {}", hex::encode(hash.into()));

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
			approvals.sort_by_key(|a| a.from.clone());

			assert_ok!(Multibatching::batch(
				RuntimeOrigin::signed(sender.clone()),
				domain,
				sender.clone().into(),
				bias,
				expires_at,
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

			let domain: [u8; 8] = *b"MYTH_NET";
			let bias = [0u8; 32];
			let expires_at =
				Timestamp::get() + <Test as pallet_timestamp::Config>::Moment::from(100_000_u64);

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
				expires_at,
				calls: calls.clone(),
				approvals: BoundedVec::new(),
			}
			.into();
			let pseudo_call_bytes = pseudo_call.encode();
			let hash = blake2_256(&pseudo_call_bytes);

			//eprintln!("test   bytes: {}", hex::encode(&pseudo_call_bytes));
			//eprintln!("test   hash: {}", hex::encode(hash.into()));

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
			approvals.sort_by_key(|a| a.from.clone());

			assert_noop!(
				Multibatching::batch(
					RuntimeOrigin::signed(sender.clone()),
					domain,
					sender.clone().into(),
					bias,
					expires_at,
					calls,
					approvals,
				),
				Error::<Test>::InvalidSignature(0)
			);
		})
	}

	#[test]
	fn multibatching_fails_with_no_signatures() {
		new_test_ext().execute_with(|| {
			let call_count = 10;
			let signer_count = 10;

			let domain: [u8; 8] = *b"MYTH_NET";
			let bias = [0u8; 32];
			let expires_at =
				Timestamp::get() + <Test as pallet_timestamp::Config>::Moment::from(100_000_u64);

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
				expires_at,
				calls: calls.clone(),
				approvals: BoundedVec::new(),
			}
			.into();
			let pseudo_call_bytes = pseudo_call.encode();
			let _hash = keccak_256(&pseudo_call_bytes);

			//eprintln!("test   bytes: {}", hex::encode(&pseudo_call_bytes));
			//eprintln!("test   hash: {}", hex::encode(hash.into()));

			let approvals = BoundedVec::new();

			assert_noop!(
				Multibatching::batch(
					RuntimeOrigin::signed(sender.clone()),
					domain,
					sender.clone().into(),
					bias,
					expires_at,
					calls,
					approvals,
				),
				Error::<Test>::NoApprovals
			);
		})
	}

	#[test]
	fn multibatching_fails_if_already_applied() {
		new_test_ext().execute_with(|| {
			let call_count = 10;
			let signer_count = 10;

			let domain: [u8; 8] = *b"MYTH_NET";
			let bias = [0u8; 32];
			let expires_at =
				Timestamp::get() + <Test as pallet_timestamp::Config>::Moment::from(100_000_u64);

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
				expires_at,
				calls: calls.clone(),
				approvals: BoundedVec::new(),
			}
			.into();
			let pseudo_call_bytes = pseudo_call.encode();
			let hash = keccak_256(&pseudo_call_bytes);

			//eprintln!("test   bytes: {}", hex::encode(&pseudo_call_bytes));
			//eprintln!("test   hash: {}", hex::encode(hash.into()));

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
			approvals.sort_by_key(|a| a.from.clone());

			assert_ok!(Multibatching::batch(
				RuntimeOrigin::signed(sender.clone()),
				domain,
				sender.clone().into(),
				bias,
				expires_at,
				calls.clone(),
				approvals.clone(),
			));
			assert_noop!(
				Multibatching::batch(
					RuntimeOrigin::signed(sender.clone()),
					domain,
					sender.clone().into(),
					bias,
					expires_at,
					calls,
					approvals,
				),
				Error::<Test>::AlreadyApplied
			);
		})
	}

	#[test]
	fn multibatching_should_fail_if_toplevel_signer_is_not_origin() {
		new_test_ext().execute_with(|| {
			let call_count = 10;
			let signer_count = 10;

			let domain: [u8; 8] = *b"MYTH_NET";
			let bias = [0u8; 32];
			let expires_at =
				Timestamp::get() + <Test as pallet_timestamp::Config>::Moment::from(100_000_u64);

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

			let wrong_sender = account(1);
			let pseudo_call: <Test as Config>::RuntimeCall = Call::<Test>::batch {
				domain,
				sender: wrong_sender.into(),
				bias,
				expires_at,
				calls: calls.clone(),
				approvals: BoundedVec::new(),
			}
			.into();
			let pseudo_call_bytes = pseudo_call.encode();
			let hash = keccak_256(&pseudo_call_bytes);

			//eprintln!("test   bytes: {}", hex::encode(&pseudo_call_bytes));
			//eprintln!("test   hash: {}", hex::encode(hash.into()));

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
			approvals.sort_by_key(|a| a.from.clone());

			assert_noop!(
				Multibatching::batch(
					RuntimeOrigin::signed(sender.clone()),
					domain,
					wrong_sender.clone().into(),
					bias,
					expires_at,
					calls,
					approvals,
				),
				Error::<Test>::BatchSenderIsNotOrigin
			);
		})
	}

	#[test]
	fn multibatching_should_fail_if_domain_is_invalid() {
		new_test_ext().execute_with(|| {
			let call_count = 10;
			let signer_count = 10;

			let domain: [u8; 8] = *b"wrongdom";
			let bias = [0u8; 32];
			let expires_at =
				Timestamp::get() + <Test as pallet_timestamp::Config>::Moment::from(100_000_u64);

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
				expires_at,
				calls: calls.clone(),
				approvals: BoundedVec::new(),
			}
			.into();
			let pseudo_call_bytes = pseudo_call.encode();
			let hash = keccak_256(&pseudo_call_bytes);

			//eprintln!("test   bytes: {}", hex::encode(&pseudo_call_bytes));
			//eprintln!("test   hash: {}", hex::encode(hash.into()));

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
			approvals.sort_by_key(|a| a.from.clone());

			assert_noop!(
				Multibatching::batch(
					RuntimeOrigin::signed(sender.clone()),
					domain,
					sender.clone().into(),
					bias,
					expires_at,
					calls,
					approvals,
				),
				Error::<Test>::InvalidDomain
			);
		})
	}

	#[test]
	fn multibatching_should_fail_if_batch_not_signed_by_any_caller() {
		new_test_ext().execute_with(|| {
			let call_count = 10;
			let signer_count = 10;

			let domain: [u8; 8] = *b"MYTH_NET";
			let bias = [0u8; 32];
			let expires_at =
				Timestamp::get() + <Test as pallet_timestamp::Config>::Moment::from(100_000_u64);

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
				expires_at,
				calls: calls.clone(),
				approvals: BoundedVec::new(),
			}
			.into();
			let pseudo_call_bytes = pseudo_call.encode();
			let hash = keccak_256(&pseudo_call_bytes);

			//eprintln!("test   bytes: {}", hex::encode(&pseudo_call_bytes));
			//eprintln!("test   hash: {}", hex::encode(hash.into()));

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
			approvals.remove(0);
			approvals.sort_by_key(|a| a.from.clone());

			assert_noop!(
				Multibatching::batch(
					RuntimeOrigin::signed(sender.clone()),
					domain,
					sender.clone().into(),
					bias,
					expires_at,
					calls,
					approvals,
				),
				Error::<Test>::InvalidCallOrigin(0)
			);
		})
	}

	#[test]
	fn multibatching_should_fail_if_caller_signature_incorrect() {
		new_test_ext().execute_with(|| {
			let call_count = 10;
			let signer_count = 10;

			let domain: [u8; 8] = *b"MYTH_NET";
			let bias = [0u8; 32];
			let expires_at =
				Timestamp::get() + <Test as pallet_timestamp::Config>::Moment::from(100_000_u64);

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
				expires_at,
				calls: calls.clone(),
				approvals: BoundedVec::new(),
			}
			.into();
			let pseudo_call_bytes = pseudo_call.encode();
			let hash = keccak_256(&pseudo_call_bytes);

			//eprintln!("test   bytes: {}", hex::encode(&pseudo_call_bytes));
			//eprintln!("test   hash: {}", hex::encode(hash.into()));

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
			// sign by wrong signer
			approvals.sort_by_key(|a| a.from.clone());
			approvals[0].signature = signers[1].0.sign_prehashed(&hash.into()).into();

			assert_noop!(
				Multibatching::batch(
					RuntimeOrigin::signed(sender.clone()),
					domain,
					sender.clone().into(),
					bias,
					expires_at,
					calls,
					approvals,
				),
				Error::<Test>::InvalidSignature(0)
			);
		})
	}

	#[test]
	fn multibatching_batch_v2_should_work() {
		new_test_ext().execute_with(|| {
			let call_count = 10;
			let signer_count = 10;

			let domain: [u8; 8] = *b"MYTH_NET";
			let bias = [0u8; 32];
			let expires_at = Timestamp::get().saturating_add(
				<Test as pallet_timestamp::Config>::Moment::from(1_000_000_000_u64),
			);

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

			let pseudo_call: <Test as Config>::RuntimeCall = Call::<Test>::batch_v2 {
				domain,
				sender: sender.into(),
				bias,
				expires_at,
				calls: calls.clone(),
				approvals: BoundedVec::new(),
			}
				.into();
			let pseudo_call_bytes = pseudo_call.encode();
			let pseudo_call_bytes = [b"<Bytes>", &pseudo_call_bytes[..], b"</Bytes>"].concat();
			let hash = keccak_256(&pseudo_call_bytes);

			//eprintln!("test   bytes: {}", hex::encode(&pseudo_call_bytes));
			//eprintln!("test   hash: {}", hex::encode(hash.into()));

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
			approvals.sort_by_key(|a| a.from.clone());

			assert_ok!(Multibatching::batch_v2(
				RuntimeOrigin::signed(sender.clone()),
				domain,
				sender.clone().into(),
				bias,
				expires_at,
				calls,
				approvals,
			));
		})
	}

	#[test]
	fn multibatching_batch_v2_fails_with_wrong_hashing() {
		new_test_ext().execute_with(|| {
			let call_count = 10;
			let signer_count = 10;

			let domain: [u8; 8] = *b"MYTH_NET";
			let bias = [0u8; 32];
			let expires_at =
				Timestamp::get() + <Test as pallet_timestamp::Config>::Moment::from(100_000_u64);

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
				expires_at,
				calls: calls.clone(),
				approvals: BoundedVec::new(),
			}
				.into();
			let pseudo_call_bytes = pseudo_call.encode();
			let pseudo_call_bytes = [b"<Bytes>", &pseudo_call_bytes[..], b"</Bytes>"].concat();
			let hash = blake2_256(&pseudo_call_bytes);

			//eprintln!("test   bytes: {}", hex::encode(&pseudo_call_bytes));
			//eprintln!("test   hash: {}", hex::encode(hash.into()));

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
			approvals.sort_by_key(|a| a.from.clone());

			assert_noop!(
				Multibatching::batch_v2(
					RuntimeOrigin::signed(sender.clone()),
					domain,
					sender.clone().into(),
					bias,
					expires_at,
					calls,
					approvals,
				),
				Error::<Test>::InvalidSignature(0)
			);
		})
	}

	#[test]
	fn multibatching_batch_v2_fails_with_no_signatures() {
		new_test_ext().execute_with(|| {
			let call_count = 10;
			let signer_count = 10;

			let domain: [u8; 8] = *b"MYTH_NET";
			let bias = [0u8; 32];
			let expires_at =
				Timestamp::get() + <Test as pallet_timestamp::Config>::Moment::from(100_000_u64);

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
				expires_at,
				calls: calls.clone(),
				approvals: BoundedVec::new(),
			}
				.into();
			let pseudo_call_bytes = pseudo_call.encode();
			let pseudo_call_bytes = [b"<Bytes>", &pseudo_call_bytes[..], b"</Bytes>"].concat();
			let _hash = keccak_256(&pseudo_call_bytes);

			//eprintln!("test   bytes: {}", hex::encode(&pseudo_call_bytes));
			//eprintln!("test   hash: {}", hex::encode(hash.into()));

			let approvals = BoundedVec::new();

			assert_noop!(
				Multibatching::batch_v2(
					RuntimeOrigin::signed(sender.clone()),
					domain,
					sender.clone().into(),
					bias,
					expires_at,
					calls,
					approvals,
				),
				Error::<Test>::NoApprovals
			);
		})
	}

	#[test]
	fn multibatching_batch_v2_fails_if_already_applied() {
		new_test_ext().execute_with(|| {
			let call_count = 10;
			let signer_count = 10;

			let domain: [u8; 8] = *b"MYTH_NET";
			let bias = [0u8; 32];
			let expires_at =
				Timestamp::get() + <Test as pallet_timestamp::Config>::Moment::from(100_000_u64);

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

			let pseudo_call: <Test as Config>::RuntimeCall = Call::<Test>::batch_v2 {
				domain,
				sender: sender.into(),
				bias,
				expires_at,
				calls: calls.clone(),
				approvals: BoundedVec::new(),
			}
				.into();
			let pseudo_call_bytes = pseudo_call.encode();
			let pseudo_call_bytes = [b"<Bytes>", &pseudo_call_bytes[..], b"</Bytes>"].concat();
			let hash = keccak_256(&pseudo_call_bytes);

			//eprintln!("test   bytes: {}", hex::encode(&pseudo_call_bytes));
			//eprintln!("test   hash: {}", hex::encode(hash.into()));

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
			approvals.sort_by_key(|a| a.from.clone());

			assert_ok!(Multibatching::batch_v2(
				RuntimeOrigin::signed(sender.clone()),
				domain,
				sender.clone().into(),
				bias,
				expires_at,
				calls.clone(),
				approvals.clone(),
			));
			assert_noop!(
				Multibatching::batch_v2(
					RuntimeOrigin::signed(sender.clone()),
					domain,
					sender.clone().into(),
					bias,
					expires_at,
					calls,
					approvals,
				),
				Error::<Test>::AlreadyApplied
			);
		})
	}

	#[test]
	fn multibatching_batch_v2_should_fail_if_toplevel_signer_is_not_origin() {
		new_test_ext().execute_with(|| {
			let call_count = 10;
			let signer_count = 10;

			let domain: [u8; 8] = *b"MYTH_NET";
			let bias = [0u8; 32];
			let expires_at =
				Timestamp::get() + <Test as pallet_timestamp::Config>::Moment::from(100_000_u64);

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

			let wrong_sender = account(1);
			let pseudo_call: <Test as Config>::RuntimeCall = Call::<Test>::batch {
				domain,
				sender: wrong_sender.into(),
				bias,
				expires_at,
				calls: calls.clone(),
				approvals: BoundedVec::new(),
			}
				.into();
			let pseudo_call_bytes = pseudo_call.encode();
			let pseudo_call_bytes = [b"<Bytes>", &pseudo_call_bytes[..], b"</Bytes>"].concat();
			let hash = keccak_256(&pseudo_call_bytes);

			//eprintln!("test   bytes: {}", hex::encode(&pseudo_call_bytes));
			//eprintln!("test   hash: {}", hex::encode(hash.into()));

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
			approvals.sort_by_key(|a| a.from.clone());

			assert_noop!(
				Multibatching::batch_v2(
					RuntimeOrigin::signed(sender.clone()),
					domain,
					wrong_sender.clone().into(),
					bias,
					expires_at,
					calls,
					approvals,
				),
				Error::<Test>::BatchSenderIsNotOrigin
			);
		})
	}

	#[test]
	fn multibatching_batch_v2_should_fail_if_domain_is_invalid() {
		new_test_ext().execute_with(|| {
			let call_count = 10;
			let signer_count = 10;

			let domain: [u8; 8] = *b"wrongdom";
			let bias = [0u8; 32];
			let expires_at =
				Timestamp::get() + <Test as pallet_timestamp::Config>::Moment::from(100_000_u64);

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
				expires_at,
				calls: calls.clone(),
				approvals: BoundedVec::new(),
			}
				.into();
			let pseudo_call_bytes = pseudo_call.encode();
			let pseudo_call_bytes = [b"<Bytes>", &pseudo_call_bytes[..], b"</Bytes>"].concat();
			let hash = keccak_256(&pseudo_call_bytes);

			//eprintln!("test   bytes: {}", hex::encode(&pseudo_call_bytes));
			//eprintln!("test   hash: {}", hex::encode(hash.into()));

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
			approvals.sort_by_key(|a| a.from.clone());

			assert_noop!(
				Multibatching::batch_v2(
					RuntimeOrigin::signed(sender.clone()),
					domain,
					sender.clone().into(),
					bias,
					expires_at,
					calls,
					approvals,
				),
				Error::<Test>::InvalidDomain
			);
		})
	}

	#[test]
	fn multibatching_batch_v2_should_fail_if_batch_not_signed_by_any_caller() {
		new_test_ext().execute_with(|| {
			let call_count = 10;
			let signer_count = 10;

			let domain: [u8; 8] = *b"MYTH_NET";
			let bias = [0u8; 32];
			let expires_at =
				Timestamp::get() + <Test as pallet_timestamp::Config>::Moment::from(100_000_u64);

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

			let pseudo_call: <Test as Config>::RuntimeCall = Call::<Test>::batch_v2 {
				domain,
				sender: sender.into(),
				bias,
				expires_at,
				calls: calls.clone(),
				approvals: BoundedVec::new(),
			}
				.into();
			let pseudo_call_bytes = pseudo_call.encode();
			let pseudo_call_bytes = [b"<Bytes>", &pseudo_call_bytes[..], b"</Bytes>"].concat();
			let hash = keccak_256(&pseudo_call_bytes);

			//eprintln!("test   bytes: {}", hex::encode(&pseudo_call_bytes));
			//eprintln!("test   hash: {}", hex::encode(hash.into()));

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
			approvals.remove(0);
			approvals.sort_by_key(|a| a.from.clone());

			assert_noop!(
				Multibatching::batch_v2(
					RuntimeOrigin::signed(sender.clone()),
					domain,
					sender.clone().into(),
					bias,
					expires_at,
					calls,
					approvals,
				),
				Error::<Test>::InvalidCallOrigin(0)
			);
		})
	}

	#[test]
	fn multibatching_batch_v2_should_fail_if_caller_signature_incorrect() {
		new_test_ext().execute_with(|| {
			let call_count = 10;
			let signer_count = 10;

			let domain: [u8; 8] = *b"MYTH_NET";
			let bias = [0u8; 32];
			let expires_at =
				Timestamp::get() + <Test as pallet_timestamp::Config>::Moment::from(100_000_u64);

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
				expires_at,
				calls: calls.clone(),
				approvals: BoundedVec::new(),
			}
				.into();
			let pseudo_call_bytes = pseudo_call.encode();
			let pseudo_call_bytes = [b"<Bytes>", &pseudo_call_bytes[..], b"</Bytes>"].concat();
			let hash = keccak_256(&pseudo_call_bytes);

			//eprintln!("test   bytes: {}", hex::encode(&pseudo_call_bytes));
			//eprintln!("test   hash: {}", hex::encode(hash.into()));

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
			// sign by wrong signer
			approvals.sort_by_key(|a| a.from.clone());
			approvals[0].signature = signers[1].0.sign_prehashed(&hash.into()).into();

			assert_noop!(
				Multibatching::batch_v2(
					RuntimeOrigin::signed(sender.clone()),
					domain,
					sender.clone().into(),
					bias,
					expires_at,
					calls,
					approvals,
				),
				Error::<Test>::InvalidSignature(0)
			);
		})
	}

}
