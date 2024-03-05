#![cfg_attr(not(feature = "std"), no_std)]

/// Edit this file to define custom logic or remove it if it is not needed.
/// Learn more about FRAME and the core library of Substrate FRAME pallets:
/// <https://docs.substrate.io/reference/frame-pallets/>
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
        traits::UnfilteredDispatchable,
        dispatch::GetDispatchInfo,
        sp_runtime::traits::{IdentifyAccount, Verify, Hash},
    };
    use frame_system::pallet_prelude::*;

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type RuntimeCall:
            Parameter
            + GetDispatchInfo
            + UnfilteredDispatchable<RuntimeOrigin = Self::RuntimeOrigin>;

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

        #[pallet::constant]
        type MaxCalls: Get<u32>;
    }

    #[pallet::error]
    pub enum Error<T> {
        InvalidOrigin,
        BatchSenderIsNotOrigin,
        InvalidCallOrigin(u16),
        InvalidSignature(u16),
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
        pub sender: T::AccountId,
        pub calls: BoundedVec<BatchedCall<T>, T::MaxCalls>,
    }

    // TODO: Is there a way to avoid manual impls here?
    impl<T: Config> Clone for Batch<T> {
        fn clone(&self) -> Self {
            Self {
                sender: self.sender.clone(),
                calls: self.calls.clone(),
            }
        }
    }

    impl<T: Config> PartialEq for Batch<T> {
        fn eq(&self, other: &Self) -> bool {
            self.sender == other.sender && self.calls == other.calls
        }
    }

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

    impl<T: Config> core::fmt::Debug for BatchedCall<T> {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> Result<(), core::fmt::Error> {
            f.debug_struct("BatchedCall")
                .field("from", &self.from)
                .field("call", &"<call>")
                .finish()
        }
    }

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
        /// If one of the calls fails, the whole batch reverts. Every participant
        /// of the batch must sign the hash of its Scale-encoded representation.
        #[pallet::call_index(0)]
        #[pallet::weight({0})] // TODO: weight
        pub fn batch(
            origin: OriginFor<T>,
            batch: Batch<T>,
            approvals: BoundedVec<Approval<T>, T::MaxCalls>,
        ) -> DispatchResult {
            if batch.calls.len() > 0 && approvals.is_empty() {
                return Err(Error::<T>::InvalidSignature(0).into());
            }

            match ensure_signed(origin) {
                Ok(account_id) if account_id == batch.sender => (),
                Ok(_) => return Err(Error::<T>::BatchSenderIsNotOrigin.into()),
                Err(_) => return Err(Error::<T>::InvalidOrigin.into()),
            }

            let hash = {
                let bytes = batch.encode();
                <T::Hashing>::hash(&bytes)
            };

            for (i, approval) in approvals.iter().enumerate() {
                let ok = approval.signature.verify(hash.as_ref(), &approval.from.clone().into_account());
                if !ok {
                    return Err(Error::<T>::InvalidSignature(i as u16).into());
                }
            }

            // TODO: count weight of batched calls

            for (i, payload) in batch.calls.into_iter().enumerate() {
                let ok = approvals
                    .iter()
                    .any(|int| int.from == payload.from);
                if !ok {
                    return Err(Error::<T>::InvalidCallOrigin(i as u16).into());
                }

                let origin = <T::RuntimeOrigin>::from(frame_system::RawOrigin::Signed(payload.from.into_account()));
                payload.call.dispatch_bypass_filter(origin).map_err(|e| e.error)?;
            }

            Ok(())
        }
    }
}
