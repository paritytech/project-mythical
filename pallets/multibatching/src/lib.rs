#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

pub mod weights;
pub use weights::*;

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::{
		dispatch::{extract_actual_weight, GetDispatchInfo, PostDispatchInfo},
		pallet_prelude::*,
		sp_runtime::traits::{Dispatchable, Hash, IdentifyAccount, Verify},
		traits::{IsSubType, OriginTrait, UnfilteredDispatchable},
	};
	use frame_system::pallet_prelude::*;
	use sp_std::vec::Vec;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		type RuntimeCall: Parameter
			+ GetDispatchInfo
			+ UnfilteredDispatchable<RuntimeOrigin = Self::RuntimeOrigin>
			+ Dispatchable<RuntimeOrigin = Self::RuntimeOrigin, PostInfo = PostDispatchInfo>
			+ From<frame_system::Call<Self>>
			+ IsSubType<Call<Self>>
			+ IsType<<Self as frame_system::Config>::RuntimeCall>;

		type Signature: Verify<Signer = Self::Signer>
			+ Clone
			+ Encode
			+ Decode
			+ Member
			+ Parameter
			+ TypeInfo;

		type Signer: IdentifyAccount<AccountId = Self::AccountId>
			+ Clone
			+ Encode
			+ Decode
			+ Parameter;

		// TODO: this should really be a u16, but the trait bound on BoundedVec
		// requires Get<u32>, which is not implemented for ConstU16.
		#[pallet::constant]
		type MaxCalls: Get<u32>;

		type WeightInfo: WeightInfo;
	}

	#[pallet::error]
	pub enum Error<T> {
		AlreadyApplied,
		BatchSenderIsNotOrigin,
		NoCalls,
		NoApprovals,
		InvalidDomain,
		DomainNotSet,
		DomainAlreadySet,
		InvalidCallOrigin(u16),
		InvalidSignature(u16),
	}

	#[pallet::storage]
	pub type Domain<T: Config> = StorageValue<_, [u8; 32], OptionQuery>;

	#[pallet::storage]
	pub type Applied<T: Config> = StorageMap<_, Identity, T::Hash, (), ValueQuery>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		BatchApplied { hash: T::Hash },
		DomainSet { domain: [u8; 32] },
	}

	/// A batch of calls.
	///
	/// Every participant that has a call in this batch must sign the hash
	/// of a batch in full, including the public key of the signer.
	///
	/// `sender` must be the origin of the `batch` extrinsic.
	#[derive(Encode, Decode, TypeInfo, MaxEncodedLen)]
	#[scale_info(skip_type_params(T))]
	pub struct Batch<T: Config> {
		pub pallet_index: u8,
		pub call_index: u8,
		pub domain: [u8; 32],
		pub sender: T::AccountId,
		pub bias: [u8; 32],
		pub calls: BoundedVec<BatchedCall<T>, T::MaxCalls>,
		pub approvals_zero: u8,
	}

	impl<T: Config> PartialEq for Batch<T> {
		fn eq(&self, other: &Self) -> bool {
			self.sender == other.sender && self.calls == other.calls
		}
	}

	// TODO: remove this before signoff
	impl<T: Config> core::fmt::Debug for Batch<T> {
		fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> Result<(), core::fmt::Error> {
			f.debug_struct("Batch")
				.field("pallet_index", &self.pallet_index)
				.field("call_index", &self.call_index)
				.field("domain", &self.domain)
				.field("sender", &self.sender)
				.field("bias", &self.bias)
				.field("calls", &self.calls)
				.field("approvals_zero", &self.approvals_zero)
				.finish()
		}
	}

	/// A call in a batch.
	#[derive(Clone, Encode, Decode, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
	#[scale_info(skip_type_params(T))]
	pub struct BatchedCall<T: Config> {
		/// The public key that will be the origin of this call.
		pub from: T::Signer,
		/// The runtime call.
		pub call: <T as Config>::RuntimeCall,
	}

	/// A signature of a batch by one of its participants.
	#[derive(Clone, Encode, Decode, PartialEq, TypeInfo, MaxEncodedLen)]
	#[scale_info(skip_type_params(T))]
	pub struct Approval<T: Config> {
		pub from: T::Signer,
		pub signature: T::Signature,
	}

	// TODO: remove this before signoff
	impl<T: Config> core::fmt::Debug for BatchedCall<T> {
		fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> Result<(), core::fmt::Error> {
			f.debug_struct("BatchedCall")
				.field("from", &self.from)
				.field("call", &"<call>")
				.finish()
		}
	}

	// TODO: remove this before signoff
	impl<T: Config> core::fmt::Debug for Approval<T> {
		fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> Result<(), core::fmt::Error> {
			f.debug_struct("Approval")
				.field("from", &self.from)
				.field("signature", &self.signature)
				.finish()
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Execute multiple calls from multiple callers in a single batch.
		///
		/// If one of the calls fails, the whole batch reverts.
		///
		/// This utility is primarily intended to support cases where the calls
		/// are interdependent - think a trade operation where Alice intends
		/// to transfer an nft item X to Bob if and only if Bob sends an nft
		/// item Y to Alice. For that reason it is designed in such a way
		/// that every caller must sign the batch as a whole instead of only
		/// their own calls. This has a pleasant side effect of reducing the
		/// execution cost compared to signing each call separately, as only
		/// one signature is required per each unique caller.
		///
		/// As the data signed by callers is a well-formed call, this allows
		/// users to validate what they're signing by just decoding the data
		/// using a third-party tool before signing them, e.g. by just going
		/// to the decode tab on the official Parachain Explorer
		/// <https://polkadot.js.org/apps/#/extrinsics/decode>.
		///
		/// # Arguments
		///
		/// - `domain` - the domain of this operation that must be unique per
		/// pallet instance across networks.
		/// - `sender` - must be the same as the sender of the transaction
		/// - `bias` - an arbitrary 32 byte array that can be used to avoid
		/// hash collisions.
		/// - `calls` - a sequence of calls to execute on behalf of their
		/// respective callers.
		/// - `approvals` - a set of signatures, one signature per a unique
		/// caller.
		///
		/// # Usage
		///
		/// - Prepare a complete `batch()` call with empty vec for `approvals`
		/// parameter.
		/// - Encode the call into scale-encoded bytes.
		/// - Form the `approvals` array by having every caller that has
		/// calls in the batch sign these bytes, one signature per caller.
		/// - Send the `batch()` call with the same data and the collected
		/// approvals.
		///
		#[pallet::call_index(0)]
		#[pallet::weight({
			let dispatch_infos = calls.iter().map(|call| call.call.get_dispatch_info()).collect::<Vec<_>>();
			let dispatch_weight = dispatch_infos.iter()
				.map(|di| di.weight)
				.fold(Weight::zero(), |total: Weight, weight: Weight| total.saturating_add(weight))
				.saturating_add(<T as Config>::WeightInfo::batch(calls.len() as u32, approvals.len() as u32));
			let dispatch_class = {
				let all_operational = dispatch_infos.iter()
					.map(|di| di.class)
					.all(|class| class == DispatchClass::Operational);
				if all_operational {
					DispatchClass::Operational
				} else {
					DispatchClass::Normal
				}
			};
			(dispatch_weight, dispatch_class)
        })]
		pub fn batch(
			origin: OriginFor<T>,
			domain: [u8; 32],
			sender: T::AccountId,
			bias: [u8; 32],
			calls: BoundedVec<BatchedCall<T>, T::MaxCalls>,
			approvals: BoundedVec<Approval<T>, T::MaxCalls>,
		) -> DispatchResultWithPostInfo {
			if calls.is_empty() {
				return Err(Error::<T>::NoCalls.into());
			}
			if approvals.is_empty() {
				return Err(Error::<T>::NoApprovals.into());
			}

			// Origin must be `sender`.
			match ensure_signed(origin) {
				Ok(account_id) if account_id == sender => account_id,
				Ok(_) => return Err(Error::<T>::BatchSenderIsNotOrigin.into()),
				Err(e) => return Err(e.into()),
			};

			match Domain::<T>::get() {
				Some(stored_domain) if stored_domain == domain => (),
				Some(_) => return Err(Error::<T>::InvalidDomain.into()),
				None => return Err(Error::<T>::DomainNotSet.into()),
			}

			let bytes = Batch {
				pallet_index: Self::index() as u8,
				call_index: 0,
				domain,
				sender: sender.clone(),
				bias,
				calls: calls.clone(),
				approvals_zero: 0,
			}
			.encode();
			let hash = <T::Hashing>::hash(&bytes);

			if Applied::<T>::contains_key(&hash) {
				return Err(Error::<T>::AlreadyApplied.into());
			}

			Applied::<T>::insert(hash, ());

			// Check the signatures.
			for (i, approval) in approvals.iter().enumerate() {
				let ok = approval
					.signature
					.verify(bytes.as_ref(), &approval.from.clone().into_account());
				if !ok {
					return Err(Error::<T>::InvalidSignature(i as u16).into());
				}
			}

			let mut weight = Weight::zero();

			let calls_len = calls.len();

			// Apply calls.
			for (i, payload) in calls.into_iter().enumerate() {
				let ok = approvals.iter().any(|int| int.from == payload.from);
				if !ok {
					return Err(Error::<T>::InvalidCallOrigin(i as u16).into());
				}

				let info = payload.call.get_dispatch_info();
				let mut origin = <T::RuntimeOrigin>::from(frame_system::RawOrigin::Signed(
					payload.from.into_account(),
				));
				origin.add_filter(move |c: &<T as frame_system::Config>::RuntimeCall| {
					let c = <T as Config>::RuntimeCall::from_ref(c);
					!matches!(c.is_sub_type(), Some(Call::batch { .. }))
				});
				let result = payload.call.dispatch(origin);
				weight = weight.saturating_add(extract_actual_weight(&result, &info));
				result.map_err(|mut err| {
					// Take the weight of this function itself into account.
					let base_weight =
						T::WeightInfo::batch(i.saturating_add(1) as u32, approvals.len() as u32);
					// Return the actual used weight + base_weight of this call.
					err.post_info = Some(base_weight + weight).into();
					err
				})?;
			}

			Self::deposit_event(Event::BatchApplied { hash });

			let base_weight = T::WeightInfo::batch(calls_len as u32, approvals.len() as u32);
			Ok(Some(base_weight.saturating_add(weight)).into())
		}

		/// Set the "domain" of this pallet instance.
		///
		/// Only callable by Root origin. The `domain` parameter in calls
		/// to `batch` must match the domain set by this call.
		#[pallet::call_index(1)]
		#[pallet::weight(<T as Config>::WeightInfo::force_set_domain())]
		pub fn force_set_domain(origin: OriginFor<T>, domain: [u8; 32]) -> DispatchResult {
			ensure_root(origin)?;

			if Domain::<T>::get() == Some(domain) {
				return Err(Error::<T>::DomainAlreadySet.into());
			}

			Domain::<T>::put(domain);
			Self::deposit_event(Event::DomainSet { domain });
			Ok(())
		}
	}
}
