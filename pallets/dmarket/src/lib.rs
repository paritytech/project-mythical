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
		/// A successful trade is executed.
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
		/// The bid with the provided parameters has already been executed.
		BidAlreadyExecuted,
		/// The ask with the provided parameters has already been executed.
		AskAlreadyExecuted,
		/// Buyer balance is not enough to pay for trade costs.
		BuyerBalanceTooLow,
		/// Bid expiration timestamp must be in the future.
		BidExpired,
		/// Ask expiration timestamp must be in the future.
		AskExpired,
		/// The signature provided by the buyer is invalid.
		InvalidBuyerSignature,
		/// The signature provided by the seller is invalid.
		InvalidSellerSignature,
		/// Same buyer and seller not allowed.
		BuyerIsSeller,
		/// Invalid Signed message.
		BadSignedMessage,
		/// Dmarket collection already set to the provided value.
		CollectionAlreadyInUse,
		/// Dmarket collection has not been set.
		CollectionNotSet,
		/// The provided Dmarket collect was not found.
		CollectionNotFound,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Sets the Dmarket collection.
		///
		/// Only the root origin can execute this function.
		///
		/// Precondition:
		/// - The collection must already exist, otherwise the extrinsic will fail.
		///
		/// Parameters:
		/// - `collection_id`: The collectionID of the NFT collection to be set as the Dmarket Collection.
		///
		///
		/// Emits CollectionUpdated when successful.
		///
		/// Weight: `WeightInfo::force_set_collection` (defined in the `Config` trait).
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

			DmarketCollection::<T>::put(collection_id);
			Self::deposit_event(Event::CollectionUpdated { collection_id });
			Ok(())
		}

		/// Execute a trade between a seller and a buyer for a specific NFT (item) in the configured DmarketCollection.
		///
		/// Preconditions:
		/// - The seller and buyer must be different accounts.
		/// - The seller must be the current owner of the NFT item.
		/// - The trade must not be expired, and signatures provided must be valid.
		///
		/// Parameters:
		/// - `origin`: The origin of the call, which must be part of the signed message of both seller and buyer.
		/// - `seller`: The account ID of the seller who owns the NFT item.
		/// - `buyer`: The account ID of the buyer who will purchase the NFT item.
		/// - `trade`: The parameters of the trade, including item details, prices, and expiration times.
		/// - `signatures`: The signatures from both the seller and buyer authorizing the trade.
		/// - `fee_address`: The account ID where the transaction fee will be transferred.
		///
		/// Signed message schema:
		/// - Ask: (domain, sender, fee_address, item, price, expiration).
		/// - Bid: (domain, sender, fee_address, item, price, fee, expiration).
		///
		/// Only callable if origin matches `sender` in both Ask and Bid signed messages.
		///
		/// Emits `Trade` event upon successful execution.
		///
		/// Weight: `WeightInfo::execute_trade` (defined in the `Config` trait).
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

			let order_data: OrderDataOf<T> =
				OrderData { caller: who, fee_address: fee_address.clone() };

			//Store closed trades
			ClosedAsks::<T>::insert(ask_hash, order_data.clone());
			ClosedBids::<T>::insert(bid_hash, order_data);

			<pallet_nfts::Pallet<T> as Transfer<T::AccountId>>::transfer(
				&collection,
				&trade.item,
				&buyer,
			)?;
			<T as crate::Config>::Currency::transfer(&buyer, &seller, trade.price, Preserve)
				.map_err(|_| Error::<T>::BuyerBalanceTooLow)?;
			<T as crate::Config>::Currency::transfer(&seller, &fee_address, trade.fee, Preserve)?;

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
			if !signature.verify(message.as_ref(), who) {
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
