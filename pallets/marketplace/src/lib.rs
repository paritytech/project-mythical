#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

mod types;
pub use types::*;

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::{
		pallet_prelude::*,
		traits::fungible::{Inspect, Mutate, MutateHold},
	};
	use frame_system::{ensure_signed, pallet_prelude::*};

	use frame_support::{dispatch::GetDispatchInfo, traits::UnfilteredDispatchable};
	use sp_std::vec::Vec;
	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config:
		frame_system::Config + pallet_nfts::Config + pallet_timestamp::Config
	{
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		type RuntimeCall: Parameter
			+ UnfilteredDispatchable<RuntimeOrigin = Self::RuntimeOrigin>
			+ GetDispatchInfo;

		/// The fungible trait use for balance holds and transfers.
		type Currency: Inspect<Self::AccountId>
			+ Mutate<Self::AccountId>
			+ MutateHold<Self::AccountId, Reason = Self::RuntimeHoldReason>;

		/// Overarching hold reason.
		type RuntimeHoldReason: From<HoldReason>;

		/// The minimum amount of time for an ask duration.
		#[pallet::constant]
		type MinOrderDuration: Get<Self::Moment>;

		/// Used for calculation of fees
		#[pallet::constant]
		type MaxBasisPoints: Get<BalanceOf<Self>>;

		/// Size of nonce StorageValue
		#[pallet::constant]
		type NonceStringLimit: Get<u32>;
	}

	/// A reason for the pallet placing a hold on funds.
	#[pallet::composite_enum]
	pub enum HoldReason {
		/// Funds are held for a created bid.
		MarketplaceBid,
	}

	#[pallet::storage]
	#[pallet::getter(fn authority)]
	pub type Authority<T: Config> = StorageValue<_, T::AccountId, OptionQuery>;

	#[pallet::storage]
	#[pallet::getter(fn fee_signer)]
	pub type FeeSigner<T: Config> = StorageValue<_, T::AccountId, OptionQuery>;

	#[pallet::storage]
	#[pallet::getter(fn payout_address)]
	pub type PayoutAddress<T: Config> = StorageValue<_, T::AccountId, OptionQuery>;

	#[pallet::storage]
	#[pallet::getter(fn nonces)]
	pub type Nonces<T: Config> =
		StorageMap<_, Identity, BoundedVec<u8, T::NonceStringLimit>, bool, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn asks)]
	pub type Asks<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::CollectionId,
		Blake2_128Concat,
		T::ItemId,
		Ask<T::AccountId, BalanceOf<T>, T::Moment>,
	>;

	#[pallet::storage]
	#[pallet::getter(fn bids)]
	pub type Bids<T: Config> = StorageNMap<
		_,
		(
			NMapKey<Blake2_128Concat, T::CollectionId>,
			NMapKey<Blake2_128Concat, T::ItemId>,
			NMapKey<Blake2_128Concat, BalanceOf<T>>,
		),
		Bid<T::AccountId, T::Moment, BalanceOf<T>>,
	>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// The pallet's authority was updated.
		AuthorityUpdated { authority: T::AccountId },
		/// The fee signer account was updated.
		FeeSignerAddressUpdate { fee_signer: T::AccountId },
		/// The payout address account was updated.
		PayoutAddressUpdated { payout_address: T::AccountId },
		/// An Ask/Bid order was created.
		OrderCreated {
			who: T::AccountId,
			order_type: OrderType,
			collection: T::CollectionId,
			item: T::ItemId,
			price: BalanceOf<T>,
			expires_at: T::Moment,
		},
		/// A trade of Ask and Bid was executed.
		OrderExecuted {
			collection: T::CollectionId,
			item: T::ItemId,
			seller: T::AccountId,
			buyer: T::AccountId,
			price: BalanceOf<T>,
		},
		/// The order was canceled by the order creator or the pallet's authority.
		OrderCanceled { collection: T::CollectionId, item: T::ItemId, who: T::AccountId },
	}

	#[pallet::error]
	pub enum Error<T> {
		/// The account is not the authority
		NotAuthority,
		/// Tried to store an account that is already set for this storage value.
		AccountAlreadySet,
		//// The fee signer address doesn't exist
		FeeSignerAddressNotSet,
		// The payout address doesn't exist
		PayoutAddressNotSet,
		/// The item was not found.
		ItemNotFound,
		/// The provided price is too low.
		InvalidPrice,
		/// Expiration time provided is too low.
		InvalidExpiration,
		/// Fee percent provided is too low.
		InvalidFeePercent,
		/// Ask or Bid with the same characteristics already exists.
		OrderAlreadyExists,
		/// Item can only be operated by the Item owner.
		NotItemOwner,
		/// Invalid Signed message
		BadSignedMessage,
		/// The Item is already locked and can't be used.
		ItemAlreadyLocked,
		/// Nonce has already been used
		AlreadyUsedNonce,
		/// The item is already owned by the account trying to bid on it.
		BidOnOwnedItem,
		/// Not allowed for the buyer of an item to be the same as the seller.
		BuyerIsSeller,
		/// The ask is already past its expiration time.
		OrderExpired,
		/// The order was not found.
		OrderNotFound,
		/// User Balance is insufficient for the required action
		InsufficientFunds,
		/// The caller is not the orderc creator or the admin account of the pallet
		NotOrderCreatorOrAdmin,
		/// The provided nonce had an invalid size
		BadNonce,
		/// An overflow happened.
		Overflow,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Sets authority role which has owner rights, its only callable by root origin
		#[pallet::call_index(0)]
		#[pallet::weight({0})]
		pub fn force_set_authority(
			origin: OriginFor<T>,
			authority: T::AccountId,
		) -> DispatchResult {
			ensure_root(origin)?;

			ensure!(
				Authority::<T>::get().as_ref() != Some(&authority),
				Error::<T>::AccountAlreadySet
			);

			Authority::<T>::put(authority.clone());
			Self::deposit_event(Event::AuthorityUpdated { authority });
			Ok(())
		}

		/// Allows authority account to set the account that signs fees.
		#[pallet::call_index(1)]
		#[pallet::weight({0})]
		pub fn set_fee_signer_address(
			origin: OriginFor<T>,
			fee_signer: T::AccountId,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			Self::ensure_authority(&who)?;

			ensure!(
				FeeSigner::<T>::get().as_ref() != Some(&fee_signer),
				Error::<T>::AccountAlreadySet
			);

			FeeSigner::<T>::put(fee_signer.clone());
			Self::deposit_event(Event::FeeSignerAddressUpdate { fee_signer });
			Ok(())
		}

		/// Allows authority account to set the payout address that receives fee payments from trades
		#[pallet::call_index(2)]
		#[pallet::weight({0})]
		pub fn set_payout_address(
			origin: OriginFor<T>,
			payout_address: T::AccountId,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			Self::ensure_authority(&who)?;

			ensure!(
				PayoutAddress::<T>::get().as_ref() != Some(&payout_address),
				Error::<T>::AccountAlreadySet
			);

			PayoutAddress::<T>::put(payout_address.clone());
			Self::deposit_event(Event::PayoutAddressUpdated { payout_address });
			Ok(())
		}

		/// Create Ask or Bid Order on an specific NFT (collectionId, ItemId).
		/// Asks:
		/// 	- An owner of an Nft can create an ask on the item wih a price, expiration and signature
		/// 	- The signature must come from the feeSigner account
		/// 	- The expiration must be above MinOrderDuration
		///     - After the ask is created the NFT is locked so it can't be transferred
		/// Bids:
		/// 	- A bid can be created on an existing item, with a price to pay, a fee, and expiration
		/// 	- The signature must come from the feeSigner account
		/// 	- The amount the bidder is willing to pay is locked from the user's Balance
		///	Match Exists
		/// 	- If a match between an Ask and Bid exists the trade is triggered
		/// 	- The seller receives the funds and the bidder receives the unlocked item
		///     - Fees go to payoutAddress
		///
		#[pallet::call_index(3)]
		#[pallet::weight({0})]
		pub fn create_order(
			origin: OriginFor<T>,
			order: Order<
				T::CollectionId,
				T::ItemId,
				BalanceOf<T>,
				T::Moment,
				T::OffchainSignature,
				Vec<u8>,
			>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			// Simulate events depending on orderTypes, just for the sake of experimenting.
			// - If orderType Ask is received then an orderCreated event is emitted
			//- If orderType Bid is received then an orderExecuted event is emitted
			match order.clone().order_type {
				OrderType::Ask => Self::deposit_event(Event::OrderCreated {
					who,
					order_type: order.order_type,
					collection: order.collection,
					item: order.item,
					price: order.price,
					expires_at: order.expires_at,
				}),
				OrderType::Bid => Self::deposit_event(Event::OrderExecuted {
					collection: order.collection,
					item: order.item,
					seller: who.clone(),
					buyer: who,
					price: order.price,
				}),
			};

			Ok(())
		}

		/// Cancelation of Ask or Bid order
		///
		/// Callable by either authority or order creator
		///
		/// If the order is an Ask the item is unlocked
		/// If the order is a Bid the bidders balance is unlocked
		#[pallet::call_index(4)]
		#[pallet::weight({0})]
		pub fn cancel_order(
			origin: OriginFor<T>,
			order_type: OrderType,
			collection: T::CollectionId,
			item: T::ItemId,
			price: BalanceOf<T>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			Self::deposit_event(Event::OrderCanceled { collection, item, who });
			Ok(())
		}
	}
	impl<T: Config> Pallet<T> {
		pub fn ensure_authority(who: &T::AccountId) -> Result<(), Error<T>> {
			match Authority::<T>::get().as_ref() == Some(who) {
				true => Ok(()),
				_ => Err(Error::<T>::NotAuthority),
			}
		}
	}
}
