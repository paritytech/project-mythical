//! # Marketplace Module
//!
//! A module that facilitates trading of non-fungible items (NFTs) through the creation and management of orders.
//!
//! ## Related Modules
//! - NFTs: Provides functionalities for managing non-fungible tokens.

#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

mod types;
pub use types::*;

pub mod weights;
pub use weights::*;

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use core::usize;

	use super::*;
	use frame_support::{
		ensure,
		pallet_prelude::*,
		traits::{
			fungible::{Inspect, Mutate, MutateHold},
			nonfungibles_v2::Transfer,
			tokens::{Precision::Exact, Preservation::Preserve},
		},
	};
	use frame_system::{ensure_signed, pallet_prelude::*};

	use frame_support::{dispatch::GetDispatchInfo, traits::UnfilteredDispatchable};

	use pallet_nfts::ItemId;
	use sp_runtime::{
		traits::{CheckedAdd, CheckedSub, IdentifyAccount, Verify},
		BoundedVec, DispatchError, Saturating,
	};
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

		/// The currency trait.
		type Currency: Inspect<Self::AccountId>
			+ Mutate<Self::AccountId>
			+ MutateHold<Self::AccountId, Reason = <Self as pallet::Config>::RuntimeHoldReason>;

		type Escrow: Escrow<Self::AccountId, BalanceOf<Self>, Self::AccountId>;

		/// Overarching hold reason.
		type RuntimeHoldReason: From<HoldReason>;

		/// The minimum amount of time for an ask duration.
		#[pallet::constant]
		type MinOrderDuration: Get<Self::Moment>;

		/// Size of nonce StorageValue
		#[pallet::constant]
		type NonceStringLimit: Get<u32>;

		/// Off-Chain signature type.
		///
		/// Can verify whether a `Self::Signer` created a signature.
		type Signature: Verify<Signer = Self::Signer> + Parameter;

		/// Off-Chain public key.
		///
		/// Must identify as an on-chain `Self::AccountId`.
		type Signer: IdentifyAccount<AccountId = Self::AccountId>;

		/// Type representing the weight of this pallet
		type WeightInfo: WeightInfo;

		#[cfg(feature = "runtime-benchmarks")]
		/// A set of helper functions for benchmarking.
		type BenchmarkHelper: BenchmarkHelper<Self::CollectionId, ItemId, Self::Moment>;
	}

	/// A reason for the pallet placing a hold on funds.
	#[pallet::composite_enum]
	pub enum HoldReason {
		/// Funds are held for a created bid.
		MarketplaceBid,
	}

	#[pallet::storage]
	pub type Authority<T: Config> = StorageValue<_, T::AccountId, OptionQuery>;

	#[pallet::storage]
	pub type FeeSigner<T: Config> = StorageValue<_, T::AccountId, OptionQuery>;

	#[pallet::storage]
	pub type Nonces<T: Config> =
		StorageMap<_, Identity, BoundedVec<u8, T::NonceStringLimit>, bool, ValueQuery>;

	#[pallet::storage]
	pub type PayoutAddress<T: Config> = StorageValue<_, T::AccountId, OptionQuery>;

	#[pallet::storage]
	pub type Asks<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::CollectionId,
		Blake2_128Concat,
		ItemId,
		Ask<T::AccountId, BalanceOf<T>, T::Moment, T::AccountId>,
	>;

	#[pallet::storage]
	pub type Bids<T: Config> = StorageNMap<
		_,
		(
			NMapKey<Blake2_128Concat, T::CollectionId>,
			NMapKey<Blake2_128Concat, ItemId>,
			NMapKey<Blake2_128Concat, BalanceOf<T>>,
		),
		Bid<T::AccountId, BalanceOf<T>, T::Moment>,
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
			item: ItemId,
			price: BalanceOf<T>,
			expires_at: T::Moment,
			fee: BalanceOf<T>,
		},
		/// A trade of Ask and Bid was executed.
		OrderExecuted {
			collection: T::CollectionId,
			item: ItemId,
			seller: T::AccountId,
			buyer: T::AccountId,
			price: BalanceOf<T>,
			seller_fee: BalanceOf<T>,
			buyer_fee: BalanceOf<T>,
		},
		/// The order was canceled by the order creator or the pallet's authority.
		OrderCanceled { collection: T::CollectionId, item: ItemId, who: T::AccountId },
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
		/// A valid match must exist to execute the order
		ValidMatchMustExist,
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
		/// Sets the authority role, granting owner rights.
		///
		/// Only the root origin can execute this function.
		///
		/// Parameters:
		/// - `authority`: The account ID of the administrator to be set as the pallet's authority.
		///
		/// Emits AuthorityUpdated when successful.
		///
		/// Weight: `WeightInfo::force_set_authority` (defined in the `Config` trait).
		#[pallet::call_index(0)]
		#[pallet::weight(<T as Config>::WeightInfo::force_set_authority())]
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

		/// Sets the fee signer address, allowing the designated account that signs fees.
		///
		/// Only an account with the authority role can execute this function. /// - `fee_signer`: The account ID of the fee signer to be set.
		///
		/// Emits `FeeSignerAddressUpdate` event upon successful execution.
		///
		/// Weight: `WeightInfo::set_fee_signer_address` (defined in the `Config` trait).
		#[pallet::call_index(1)]
		#[pallet::weight(<T as Config>::WeightInfo::set_fee_signer_address())]
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

		/// Allows the authority account to set the payout address, which receives fee payments from trades.
		///
		/// Only an account with the authority role can execute this function.
		///
		/// Parameters:
		/// - `payout_address`: The account ID of the address to be set as the payout address.
		///
		/// Emits `PayoutAddressUpdated` event upon successful execution.
		///
		/// Weight: `WeightInfo::set_payout_address` (defined in the `Config` trait).
		#[pallet::call_index(2)]
		#[pallet::weight(<T as Config>::WeightInfo::set_payout_address())]
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

		/// Create an Ask or Bid Order for a specific NFT (collection ID, Item ID).
		///
		/// Asks:
		/// - An owner of an NFT can create an ask on the item with a price, expiration, and signature.
		/// - The signature must come from the feeSigner account.
		/// - The expiration must be above `MinOrderDuration`.
		/// - After the ask is created, the NFT is locked so it can't be transferred.
		///
		/// Bids:
		/// - A bid can be created on an existing item, with a price to pay, a fee, and expiration.
		/// - The signature must come from the feeSigner account.
		/// - The amount the bidder is willing to pay is locked from the user's Balance.
		///
		/// Match Exists:
		/// - If a match between an Ask and Bid exists, the trade is triggered.
		/// - The seller receives the funds, and the bidder receives the unlocked item.
		/// - Fees go to payoutAddress.
		///
		/// Parameters:
		/// - `order`: The details of the order to be created (including type, collection, item, price, expiration, fee, and signature).
		/// - `execution`: Execution mode to indicate whether order creation should proceed if a valid match exists.
		///
		/// Emits `OrderCreated` event upon successful execution.
		///
		/// Weight: `WeightInfo::create_order` (defined in the `Config` trait).
		#[pallet::call_index(3)]
		#[pallet::weight(<T as Config>::WeightInfo::create_order())]
		pub fn create_order(
			origin: OriginFor<T>,
			order: OrderOf<T>,
			execution: Execution,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let item_owner = pallet_nfts::Pallet::<T>::owner(order.collection, order.item)
				.ok_or(Error::<T>::ItemNotFound)?;

			ensure!(
				order.expires_at
					> pallet_timestamp::Pallet::<T>::get()
						.saturating_add(T::MinOrderDuration::get()),
				Error::<T>::InvalidExpiration
			);

			let message: OrderMessageOf<T> = order.clone().into();
			Self::verify_fee_signer_signature(&message.encode(), order.signature_data)?;

			Self::deposit_event(Event::OrderCreated {
				who: who.clone(),
				order_type: order.order_type.clone(),
				collection: order.collection,
				item: order.item,
				price: order.price,
				expires_at: order.expires_at,
				fee: order.fee,
			});

			match order.order_type {
				OrderType::Ask => {
					ensure!(
						!Asks::<T>::contains_key(order.collection, order.item),
						Error::<T>::OrderAlreadyExists
					);
					ensure!(item_owner == who.clone(), Error::<T>::NotItemOwner);
					//Check if item is locked
					pallet_nfts::Pallet::<T>::disable_transfer(&order.collection, &order.item)
						.map_err(|_| Error::<T>::ItemAlreadyLocked)?;

					if let Some(exec_order) = Self::valid_match_exists_for(
						OrderType::Ask,
						&order.collection,
						&order.item,
						&order.price,
					) {
						Self::execute_order(
							exec_order,
							who,
							order.collection,
							order.item,
							&order.price,
							&order.fee,
							order.escrow_agent,
						)?;
					} else {
						ensure!(
							execution == Execution::AllowCreation,
							Error::<T>::ValidMatchMustExist
						);

						let ask = Ask {
							seller: who,
							price: order.price,
							expiration: order.expires_at,
							fee: order.fee,
							escrow_agent: order.escrow_agent,
						};

						Asks::<T>::insert(order.collection, order.item, ask);
					}
				},

				OrderType::Bid => {
					ensure!(
						!Bids::<T>::contains_key((order.collection, order.item, order.price)),
						Error::<T>::OrderAlreadyExists
					);
					ensure!(item_owner != who.clone(), Error::<T>::BidOnOwnedItem);

					//Reserve neccesary amount to pay for the item + fees
					let bid_payment = Self::calc_bid_payment(&order.price, &order.fee)?;
					<T as crate::Config>::Currency::hold(
						&HoldReason::MarketplaceBid.into(),
						&who,
						bid_payment,
					)
					.map_err(|_| Error::<T>::InsufficientFunds)?;

					if let Some(exec_order) = Self::valid_match_exists_for(
						OrderType::Bid,
						&order.collection,
						&order.item,
						&order.price,
					) {
						Self::execute_order(
							exec_order,
							who,
							order.collection,
							order.item,
							&order.price,
							&order.fee,
							order.escrow_agent,
						)?;
					} else {
						ensure!(
							execution == Execution::AllowCreation,
							Error::<T>::ValidMatchMustExist
						);

						let bid = Bid { buyer: who, expiration: order.expires_at, fee: order.fee };

						Bids::<T>::insert((order.collection, order.item, order.price), bid);
					}
				},
			};

			Ok(())
		}

		/// Cancelation of an Ask or Bid order.
		///
		/// Callable by either the authority or the order creator.
		///
		/// If the order is an Ask, the item is unlocked.
		/// If the order is a Bid, the bidder's balance is unlocked.
		///
		/// Parameters:
		/// - `order_type`: The type of the order to be canceled (Ask or Bid).
		/// - `collection`: The collection ID of the NFT associated with the order.
		/// - `item`: The item ID of the NFT associated with the order.
		/// - `price`: The price associated with the order (used for Bid orders).
		///
		/// Emits `OrderCanceled` event upon successful execution.
		///
		/// Weight: `WeightInfo::cancel_order` (defined in the `Config` trait).
		#[pallet::call_index(4)]
		#[pallet::weight(<T as Config>::WeightInfo::cancel_order())]
		pub fn cancel_order(
			origin: OriginFor<T>,
			order_type: OrderType,
			collection: T::CollectionId,
			item: ItemId,
			price: BalanceOf<T>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let authority = Authority::<T>::get();

			match order_type {
				//Order type Ask
				OrderType::Ask => {
					let ask = Asks::<T>::get(collection, item).ok_or(Error::<T>::OrderNotFound)?;
					ensure!(
						ask.seller.clone() == who.clone() || Some(who.clone()) == authority,
						Error::<T>::NotOrderCreatorOrAdmin
					);

					Asks::<T>::remove(collection, item);

					// Re enable item transfer
					pallet_nfts::Pallet::<T>::enable_transfer(&collection, &item)?;
				},
				//Order type Bid
				OrderType::Bid => {
					let bid = Bids::<T>::get((collection, item, price))
						.ok_or(Error::<T>::OrderNotFound)?;

					ensure!(
						bid.buyer.clone() == who.clone() || Some(who.clone()) == authority,
						Error::<T>::NotOrderCreatorOrAdmin
					);

					Bids::<T>::remove((collection, item, price));

					let bid_payment = Self::calc_bid_payment(&price, &bid.fee)?;
					<T as crate::Config>::Currency::release(
						&HoldReason::MarketplaceBid.into(),
						&bid.buyer,
						bid_payment,
						Exact,
					)?;
				},
			}

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

		pub fn valid_match_exists_for(
			order_type: OrderType,
			collection: &T::CollectionId,
			item: &ItemId,
			price: &BalanceOf<T>,
		) -> Option<ExecOrder<T::AccountId, BalanceOf<T>, T::Moment, T::AccountId>> {
			let timestamp = pallet_timestamp::Pallet::<T>::get();

			match order_type {
				OrderType::Ask => {
					if let Some(bid) = Bids::<T>::get((collection, item, price)) {
						if timestamp >= bid.expiration {
							return None;
						};
						return Some(ExecOrder::Bid(bid));
					}
				},
				OrderType::Bid => {
					if let Some(ask) = Asks::<T>::get(collection, item) {
						if timestamp >= ask.expiration || ask.price != *price {
							return None;
						};
						return Some(ExecOrder::Ask(ask));
					}
				},
			}
			None
		}

		pub fn execute_order(
			exec_order: ExecOrder<T::AccountId, BalanceOf<T>, T::Moment, T::AccountId>,
			who: T::AccountId,
			collection: T::CollectionId,
			item: ItemId,
			price: &BalanceOf<T>,
			fee: &BalanceOf<T>,
			order_escrow_agent: Option<T::AccountId>,
		) -> Result<(), DispatchError> {
			let seller: T::AccountId;
			let buyer: T::AccountId;
			let seller_fee: BalanceOf<T>;
			let buyer_fee: BalanceOf<T>;
			let escrow_agent: Option<T::AccountId>;

			match exec_order {
				ExecOrder::Bid(bid) => {
					ensure!(who.clone() != bid.buyer.clone(), Error::<T>::BuyerIsSeller);

					seller = who;
					buyer = bid.buyer;
					seller_fee = *fee;
					buyer_fee = bid.fee;
					escrow_agent = order_escrow_agent;
				},
				ExecOrder::Ask(ask) => {
					ensure!(who.clone() != ask.seller.clone(), Error::<T>::BuyerIsSeller);

					seller = ask.seller;
					buyer = who;
					seller_fee = ask.fee;
					buyer_fee = *fee;
					escrow_agent = ask.escrow_agent;
				},
			};

			Asks::<T>::remove(collection, item);
			Bids::<T>::remove((collection, item, *price));

			Self::process_fees(&seller, seller_fee, &buyer, buyer_fee, *price, escrow_agent)?;

			pallet_nfts::Pallet::<T>::enable_transfer(&collection, &item)?;
			<pallet_nfts::Pallet<T> as Transfer<T::AccountId>>::transfer(
				&collection,
				&item,
				&buyer,
			)?;

			Self::deposit_event(Event::OrderExecuted {
				collection,
				item,
				seller,
				buyer,
				price: *price,
				seller_fee,
				buyer_fee,
			});
			Ok(())
		}

		pub fn calc_bid_payment(
			price: &BalanceOf<T>,
			fee: &BalanceOf<T>,
		) -> Result<BalanceOf<T>, Error<T>> {
			price.checked_add(&fee).ok_or(Error::<T>::Overflow)
		}

		pub fn process_fees(
			seller: &T::AccountId,
			seller_fee: BalanceOf<T>,
			buyer: &T::AccountId,
			buyer_fee: BalanceOf<T>,
			price: BalanceOf<T>,
			escrow_agent: Option<T::AccountId>,
		) -> Result<(), DispatchError> {
			//Amount to be payed by the buyer
			let buyer_payment_amount = price.checked_add(&buyer_fee).ok_or(Error::<T>::Overflow)?;

			//Amount to be payed to the marketplace at the payoutAddress
			let marketplace_pay_amount =
				buyer_fee.checked_add(&seller_fee).ok_or(Error::<T>::Overflow)?;

			//Amount to be payed to the seller (Earings - marketFees)
			let seller_pay_amount = buyer_payment_amount
				.checked_sub(&marketplace_pay_amount)
				.ok_or(Error::<T>::Overflow)?;

			<T as crate::Config>::Currency::release(
				&HoldReason::MarketplaceBid.into(),
				buyer,
				buyer_payment_amount,
				Exact,
			)?;
			// Pay fees to PayoutAddress
			let payout_address =
				PayoutAddress::<T>::get().ok_or(Error::<T>::PayoutAddressNotSet)?;
			<T as crate::Config>::Currency::transfer(
				buyer,
				&payout_address,
				marketplace_pay_amount,
				Preserve,
			)?;
			//Pay earnings to seller
			match escrow_agent {
				Some(agent) => {
					T::Escrow::make_deposit(buyer, seller, seller_pay_amount, &agent)?;
				},
				None => {
					<T as crate::Config>::Currency::transfer(
						buyer,
						seller,
						seller_pay_amount,
						Preserve,
					)?;
				},
			}

			Ok(())
		}

		fn verify_fee_signer_signature(
			message: &Vec<u8>,
			signature_data: SignatureData<T::Signature, Vec<u8>>,
		) -> Result<(), DispatchError> {
			let nonce: BoundedVec<u8, T::NonceStringLimit> =
				signature_data.nonce.try_into().map_err(|_| Error::<T>::BadNonce)?;

			ensure!(!Nonces::<T>::contains_key(nonce.clone()), Error::<T>::AlreadyUsedNonce);

			let signer = FeeSigner::<T>::get().ok_or(Error::<T>::FeeSignerAddressNotSet)?;

			if !signature_data.signature.verify(message.as_ref(), &signer) {
				return Err(Error::<T>::BadSignedMessage.into());
			}

			Nonces::<T>::set(nonce, true);
			Ok(())
		}
	}
}

sp_core::generate_feature_enabled_macro!(runtime_benchmarks_enabled, feature = "runtime-benchmarks", $);
