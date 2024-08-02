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
use parity_scale_codec::Codec;
pub use types::*;

pub mod weights;
pub use weights::*;

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use core::usize;

	use crate::Item;

	use super::*;
	use frame_support::{
		ensure,
		pallet_prelude::*,
		traits::{
			fungible::{Inspect, Mutate},
			nonfungibles_v2::{Inspect as NftInspect, Transfer},
			tokens::Preservation::Preserve,
		},
	};
	use frame_system::{ensure_signed, pallet_prelude::*};

	use frame_support::{dispatch::GetDispatchInfo, traits::UnfilteredDispatchable};

	use sp_runtime::traits::Hash;
	use sp_runtime::{
		traits::{IdentifyAccount, Verify},
		DispatchError,
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
		type Currency: Inspect<Self::AccountId> + Mutate<Self::AccountId>;

		/// Off-Chain signature type.
		///
		/// Can verify whether a `Self::Signer` created a signature.
		type Signature: Verify<Signer = Self::Signer> + Parameter;

		/// Off-Chain public key.
		///
		/// Must identify as an on-chain `Self::AccountId`.
		type Signer: IdentifyAccount<AccountId = Self::AccountId>;

		///Chain Domain
		#[pallet::constant]
		type Domain: Get<Domain>;

		/// Type representing the weight of this pallet
		type WeightInfo: WeightInfo;

		#[cfg(feature = "runtime-benchmarks")]
		/// A set of helper functions for benchmarking.
		type BenchmarkHelper: BenchmarkHelper<Self::CollectionId, Self::Moment>;
	}

	#[pallet::storage]
	pub type ClosedAsks<T: Config> = StorageMap<_, Blake2_128Concat, T::Hash, OrderDataOf<T>>;

	#[pallet::storage]
	pub type ClosedBids<T: Config> = StorageMap<_, Blake2_128Concat, T::Hash, OrderDataOf<T>>;

	#[pallet::storage]
	pub type DmarketCollection<T: Config> = StorageValue<_, T::CollectionId, OptionQuery>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// The fee signer account was updated.
		CollectionUpdated { collection_id: T::CollectionId },
		///
		Trade {
			buyer: T::AccountId,
			seller: T::AccountId,
			item: Item,
			price: BalanceOf<T>,
			fee: BalanceOf<T>,
		},
	}

	#[pallet::error]
	pub enum Error<T> {
		/// The item was not found.
		ItemNotFound,
		/// Item can only be operated by the Item owner.
		SellerNotItemOwner,
		///
		BidAlreadyExecuted,
		///
		AskAlreadyExecuted,
		///
		BuyerBalanceTooLow,
		///
		BidExpired,
		///
		AskExpired,
		///
		InvalidBuyerSignature,
		///
		InvalidSellerSignature,
		/// Same buyer and seller not allowed.
		BuyerIsSeller,
		///
		BadSignedMessage,
		///
		CollectionAlreadyInUse,
		///
		CollectionNotSet,
		///
		CollectionNotFound,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::call_index(0)]
		#[pallet::weight(<T as Config>::WeightInfo::force_set_collection())]
		pub fn force_set_collection(
			origin: OriginFor<T>,
			collection_id: T::CollectionId,
		) -> DispatchResult {
			ensure_root(origin)?;

			<pallet_nfts::Pallet<T> as NftInspect<T::AccountId>>::collection_owner(&collection_id)
				.ok_or(Error::<T>::CollectionNotFound)?;

			ensure!(
				DmarketCollection::<T>::get().as_ref() != Some(&collection_id),
				Error::<T>::CollectionAlreadyInUse
			);

			DmarketCollection::<T>::put(collection_id.clone());
			Self::deposit_event(Event::CollectionUpdated { collection_id });
			Ok(())
		}

		#[pallet::call_index(1)]
		#[pallet::weight(<T as Config>::WeightInfo::execute_trade())]
		pub fn execute_trade(
			origin: OriginFor<T>,
			seller: T::AccountId,
			buyer: T::AccountId,
			trade: TradeParamsOf<T>,
			signatures: TradeSignatures<<T as Config>::Signature>,
			fee_address: T::AccountId,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			ensure!(seller != buyer, Error::<T>::BuyerIsSeller);

			let timestamp = pallet_timestamp::Pallet::<T>::get();
			ensure!(trade.ask_expiration > timestamp, Error::<T>::AskExpired);
			ensure!(trade.bid_expiration > timestamp, Error::<T>::BidExpired);

			// Pay fees to FeeAddress
			let collection = DmarketCollection::<T>::get().ok_or(Error::<T>::CollectionNotSet)?;

			let item_owner = pallet_nfts::Pallet::<T>::owner(collection, trade.item)
				.ok_or(Error::<T>::ItemNotFound)?;
			ensure!(seller == item_owner, Error::<T>::SellerNotItemOwner);

			let (ask_hash, bid_hash) = Self::hash_ask_bid_data(&trade);
			ensure!(!ClosedAsks::<T>::contains_key(ask_hash), Error::<T>::AskAlreadyExecuted);
			ensure!(!ClosedBids::<T>::contains_key(bid_hash), Error::<T>::BidAlreadyExecuted);

			Self::verify_signature(
				&seller,
				&Self::get_ask_message(&who, &fee_address, &trade),
				signatures.ask_signature,
			)
			.map_err(|_| Error::<T>::InvalidSellerSignature)?;

			Self::verify_signature(
				&buyer,
				&Self::get_bid_message(&who, &fee_address, &trade),
				signatures.bid_signature,
			)
			.map_err(|_| Error::<T>::InvalidBuyerSignature)?;

			<pallet_nfts::Pallet<T> as Transfer<T::AccountId>>::transfer(
				&collection,
				&trade.item,
				&buyer,
			)?;
			<T as crate::Config>::Currency::transfer(&buyer, &seller, trade.price, Preserve)
				.map_err(|_| Error::<T>::BuyerBalanceTooLow)?;
			<T as crate::Config>::Currency::transfer(&seller, &fee_address, trade.fee, Preserve)?;

			let order_data: OrderDataOf<T> = OrderData { caller: who, fee_address };

			//Store closed trades
			ClosedAsks::<T>::insert(ask_hash, order_data.clone());
			ClosedBids::<T>::insert(bid_hash, order_data);

			Self::deposit_event(Event::Trade {
				seller,
				buyer,
				item: trade.item,
				price: trade.price,
				fee: trade.fee,
			});

			Ok(())
		}
	}

	impl<T: Config> Pallet<T> {
		fn verify_signature(
			who: &T::AccountId,
			message: &Vec<u8>,
			signature: T::Signature,
		) -> Result<(), DispatchError> {
			if !signature.verify(message.as_ref(), &who) {
				return Err(Error::<T>::BadSignedMessage.into());
			}

			Ok(())
		}

		pub fn hash_ask_bid_data(trade: &TradeParamsOf<T>) -> (T::Hash, T::Hash) {
			let ask_hash =
				T::Hashing::hash(&(trade.item, trade.price, trade.ask_expiration).encode());
			let bid_hash = T::Hashing::hash(
				&(trade.item, trade.price, trade.fee, trade.bid_expiration).encode(),
			);

			(ask_hash, bid_hash)
		}

		pub fn get_ask_message(
			caller: &T::AccountId,
			fee_address: &T::AccountId,
			trade: &TradeParamsOf<T>,
		) -> Vec<u8> {
			AskMessage {
				domain: T::Domain::get(),
				sender: caller.clone(),
				fee_address: fee_address.clone(),
				item: trade.item,
				price: trade.price,
				expiration: trade.ask_expiration,
			}
			.encode()
		}

		pub fn get_bid_message(
			caller: &T::AccountId,
			fee_address: &T::AccountId,
			trade: &TradeParamsOf<T>,
		) -> Vec<u8> {
			BidMessage {
				domain: T::Domain::get(),
				sender: caller.clone(),
				fee_address: fee_address.clone(),
				item: trade.item,
				price: trade.price,
				fee: trade.fee,
				expiration: trade.bid_expiration,
			}
			.encode()
		}
	}
}

sp_core::generate_feature_enabled_macro!(runtime_benchmarks_enabled, feature = "runtime-benchmarks", $);

use sp_std::vec::Vec;
sp_api::decl_runtime_apis! {
	pub trait DmarketApi<AccountId, Balance, Moment, Hash>
	where
		AccountId: Codec,
		Balance: Codec,
		Moment: Codec,
		Hash: Codec,
	{
		fn hash_ask_bid_data(trade: TradeParams<Balance, Item, Moment>)-> (Hash, Hash);
		fn get_ask_message(caller: AccountId, fee_address: AccountId, trade: TradeParams<Balance, Item, Moment>) -> Vec<u8>;
		fn get_bid_message(caller: AccountId, fee_address: AccountId, trade: TradeParams<Balance, Item, Moment>) -> Vec<u8>;
	}
}
