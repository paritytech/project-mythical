#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::{
        pallet_prelude::*,
        dispatch::{extract_actual_weight, GetDispatchInfo, PostDispatchInfo},
        traits::{UnfilteredDispatchable, IsSubType, OriginTrait},
        sp_runtime::traits::{IdentifyAccount, Verify, Hash, Dispatchable},
    };
    use frame_system::{
        pallet_prelude::*,
        WeightInfo,
    };

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        type RuntimeCall:
            Parameter
            + GetDispatchInfo
            + UnfilteredDispatchable<RuntimeOrigin = Self::RuntimeOrigin>
            + Dispatchable<RuntimeOrigin = Self::RuntimeOrigin, PostInfo = PostDispatchInfo>
            + From<frame_system::Call<Self>>
            + IsSubType<Call<Self>>
            + IsType<<Self as frame_system::Config>::RuntimeCall>;

        type Signature:
            Verify<Signer = Self::Signer>
            + Clone
            + Encode
            + Decode
            + Member
            + Parameter
            + TypeInfo;

        type Signer:
            IdentifyAccount<AccountId = Self::AccountId>
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
        InvalidCallOrigin(u16),
        InvalidSignature(u16),
    }

    #[pallet::storage]
    #[pallet::getter(fn domain)]
    pub type Domain<T: Config> = StorageValue<_, [u8;32], OptionQuery>;

    #[pallet::storage]
    #[pallet::getter(fn applied)]
    pub type Applied<T: Config> = StorageMap<_, Identity, T::Hash, (), ValueQuery>;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        BatchApplied {
            hash: T::Hash
        },
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
                .field("sender", &self.sender)
                .field("calls", &self.calls)
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
        /// Execute multiple calls from multiple senders in a batch atomically.
        ///
        /// If one of the calls fails, the whole batch reverts.
        ///
        /// For every unique call origin, `approvals` must contain a signature
        /// of keccak256 hash of this call with empty `approvals` argument.
        /// This is so that the participants could check the data they sign
        /// through the Developer/Extrinsics/Decode tab on the parachain
        /// explorer: https://polkadot.js.org/apps/#/extrinsics/decode
        #[pallet::call_index(0)]
        #[pallet::weight({0})] // TODO: weight
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

            let hash = {
                let batch = Batch {
                    pallet_index: Self::index() as u8,
                    call_index: 0,
                    domain,
                    sender: sender.clone(),
                    bias,
                    calls: calls.clone(),
                    approvals_zero: 0,
                };
                let bytes = batch.encode();
                <T::Hashing>::hash(&bytes)
            };

            if Applied::<T>::contains_key(&hash) {
                return Err(Error::<T>::AlreadyApplied.into());
            }

            Applied::<T>::insert(hash, ());

            for (i, approval) in approvals.iter().enumerate() {
                let ok = approval.signature.verify(hash.as_ref(), &approval.from.clone().into_account());
                if !ok {
                    return Err(Error::<T>::InvalidSignature(i as u16).into());
                }
            }


            let mut weight = Weight::zero();

            for (i, payload) in calls.into_iter().enumerate() {
                let ok = approvals
                    .iter()
                    .any(|int| int.from == payload.from);
                if !ok {
                    return Err(Error::<T>::InvalidCallOrigin(i as u16).into());
                }

                let info = payload.call.get_dispatch_info();
                let mut origin = <T::RuntimeOrigin>::from(frame_system::RawOrigin::Signed(payload.from.into_account()));
                origin.add_filter(move |c: &<T as frame_system::Config>::RuntimeCall| {
                    let c = <T as Config>::RuntimeCall::from_ref(c);
                    !matches!(c.is_sub_type(), Some(Call::batch { .. }))
                });
                let result = payload.call.dispatch(origin);
                weight = weight.saturating_add(extract_actual_weight(&result, &info));
				result.map_err(|mut err| {
					// Take the weight of this function itself into account.
					// let base_weight = T::WeightInfo::batch_all(i.saturating_add(1) as u32); // TODO
                    let base_weight = Weight::zero();
					// Return the actual used weight + base_weight of this call.
					err.post_info = Some(base_weight + weight).into();
					err
				})?;
            }

            Self::deposit_event(Event::BatchApplied { hash });

			//let base_weight = T::WeightInfo::batch_all(calls_len as u32); // TODO
			let base_weight = Weight::zero();
			Ok(Some(base_weight.saturating_add(weight)).into())
        }
    }
}
