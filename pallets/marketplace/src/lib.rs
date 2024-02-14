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
		traits::{Incrementable, LockableCurrency},
	};
	use frame_system::{
		ensure_signed,
		pallet_prelude::{BlockNumberFor, *},
	};

	use frame_support::{dispatch::GetDispatchInfo, traits::UnfilteredDispatchable};
	use sp_runtime::Permill;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_nfts::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		type RuntimeCall: Parameter
			+ UnfilteredDispatchable<RuntimeOrigin = Self::RuntimeOrigin>
			+ GetDispatchInfo;

		/// The currency trait.
		type Currency: LockableCurrency<Self::AccountId>;

		/// The minimum amount of time for an ask duration.
		#[pallet::constant]
		type MinOrderDuration: Get<BlockNumberFor<Self>>;

		/// Used for calculation of fees
		#[pallet::constant]
		type MaxBasisPoints: Get<u128>;

		/// Maximum amount of items allowed for a suggestion and wantAsk in an exchange
		#[pallet::constant]
		type MaxExchangeItems: Get<u32>;

		/// Size of nonce StorageValue
		#[pallet::constant]
		type NonceStringLimit: Get<u32>;
	}

	#[pallet::storage]
	#[pallet::getter(fn authority)]
	pub type Authority<T: Config> = StorageValue<_, T::AccountId, OptionQuery>;

	#[pallet::storage]
	#[pallet::getter(fn fee_signer)]
	pub type FeeSigner<T: Config> = StorageValue<_, T::AccountId, OptionQuery>;

	#[pallet::storage]
	#[pallet::getter(fn nonces)]
	pub type Nonces<T: Config> =
		StorageMap<_, Identity, BoundedVec<u8, T::NonceStringLimit>, bool, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn payout_address)]
	pub type PayoutAddress<T: Config> = StorageValue<_, T::AccountId, OptionQuery>;

	#[pallet::storage]
	#[pallet::getter(fn asks)]
	pub type Asks<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::CollectionId,
		Blake2_128Concat,
		T::ItemId,
		Ask<T::AccountId, BalanceOf<T>, BlockNumberFor<T>>,
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
		Bid<T::AccountId, BlockNumberFor<T>>,
	>;

	#[pallet::storage]
	#[pallet::getter(fn suggestions)]
	pub type Suggestions<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		HashId,
		Suggestion<T::CollectionId, T::ItemId, BalanceOf<T>, T::AccountId>,
		OptionQuery,
	>;

	#[pallet::storage]
	#[pallet::getter(fn want_asks)]
	pub type WantAsks<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		HashId,
		WantAsk<T::CollectionId, T::ItemId, BalanceOf<T>>,
		OptionQuery,
	>;

	#[pallet::storage]
	#[pallet::getter(fn exchanges)]
	pub type Exchanges<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		HashId,
		Exchange<T::AccountId, BlockNumberFor<T>, BalanceOf<T>>,
		OptionQuery,
	>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// The pallet's authority was updated.
		AuthorityUpdated {
			authority: T::AccountId,
		},
		/// The fee signer account was updated.
		FeeSignerAddressUpdate {
			fee_signer: T::AccountId,
		},
		/// The payout address account was updated.
		PayoutAddressUpdated {
			payout_address: T::AccountId,
		},
		/// An Ask/Bid order was created.
		OrderCreated {
			who: T::AccountId,
			order: Order<
				T::CollectionId,
				T::ItemId,
				BalanceOf<T>,
				BlockNumberFor<T>,
				BoundedVec<u8, T::NonceStringLimit>,
			>,
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
		OrderCanceled {
			collection: T::CollectionId,
			item: T::ItemId,
			who: T::AccountId,
		},
		/// An exchange was created.
		ExchangeCreated {
			who: T::AccountId,
			exchange: HashId,
		},
		/// An exchange suggestion was filled
		SuggestionFilled {
			who: T::AccountId,
			exchange: HashId,
			collection: T::CollectionId,
			item: T::ItemId,
		},
		// An exchange was executed.
		ExchangeExecuted(HashId),
		/// A suggestion was canceled by its creator.
		/// TODO: change item type after mock
		SuggestionCanceled {
			who: T::AccountId,
			exchange: HashId,
			collection: T::CollectionId,
			item: T::CollectionId, //Should be ItemId but this type doesn't have any default values that can be used to simulate the event
		},
		/// The exchange was canceled by its creator.
		ExchangeCanceled {
			who: T::AccountId,
			exchange: HashId,
		},
	}

	#[pallet::error]
	pub enum Error<T> {
		/// The account is not the authority
		NotAuthority,
		/// Tried to store an account that is already set for this storage value.
		AccountAlreadySet,
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
		/// The Item is already locked and can't be used.
		ItemAlreadyLocked,
		/// The item is already owned by the account trying to bid on it.
		BidOnOwnedItem,
		/// Invalid payment amount, should be higher than MaxBasisPoints.
		InvalidPayAmount,
		/// Not allowed for the buyer of an item to be the same as the seller.
		BuyerIsSeller,
		/// The ask is already past its expiration time.
		AskExpired,
		/// The order was not found.
		OrderNotFound,
		/// Exchange suggestion has already been filled.
		AlreadyFilled,
		/// Not enough balance to pay for bid.
		BalanceInsufficient,
		/// The exchange with the specified hash was not found.
		ExchangeNotFound,
		/// The exchange is already past its expiration time.
		ExchangeExpired,
		/// The exchange suggestion item has not been filled.
		ItemNotFilled,
		/// The item is unavailable.
		ItemUnavailable,
		/// The exchange has already been executed.
		AlreadyExecuted,
		/// The exchange is canceled.
		AlreadyCanceled,
		/// No suggesions were provided.
		SuggestionsEmpty,
		/// No wantAsks were provided.
		WantAsksEmpty,
		/// An exchange with the same caracteristics already exists.
		ExchangeAlreadyExists,
		/// Duplicated item in exchange suggestions or wasnAsks.
		ItemAlreadyInExchange,
		/// The wanted item is not available as an Ask.
		AskNotInMarketplace,
		/// Sugggestions will not generate enough value to exchange for suggested asks.
		NotEnoughValueForExchange,
		///Internal Error - TODO: Remove after mock
		InternalError,
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
			ensure_signed(origin)?;

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
			ensure_signed(origin)?;

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
				BlockNumberFor<T>,
				BoundedVec<u8, T::NonceStringLimit>,
			>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			// Simulate events depending on orderTypes, just for the sake of experimenting.
			// - If orderType Ask is received then an orderCreated event is emitted
			//- If orderType Bid is received then an orderExecuted event is emitted
			match order.clone().order_type {
				OrderType::Ask => Self::deposit_event(Event::OrderCreated { who, order }),
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
		///
		/// Distinction between ask an bid determined by wheter the price is specified (Bid) or not (Ask)
		#[pallet::call_index(4)]
		#[pallet::weight({0})]
		pub fn cancel_order(
			origin: OriginFor<T>,
			collection: T::CollectionId,
			item: T::ItemId,
			price: Option<BalanceOf<T>>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			Self::deposit_event(Event::OrderCanceled { collection, item, who });
			Ok(())
		}

		/// Create an exchange with the NFTs that the user is looking to sell and the NFTs it's looking to buy with the profit of the sale.
		/// - The offered items (suggestions) get locked from transfer
		/// - The wanted asks must already exist in the marketplace as Asks
		/// - The expiration must be above MinOrderDuration
		/// - Can specify an initial_amount to lock from the users balance. This will later be used to purchase the items.
		#[pallet::call_index(5)]
		#[pallet::weight({0})]
		pub fn create_exchange(
			origin: OriginFor<T>,
			suggestions: BoundedVec<
				Suggestion<T::CollectionId, T::ItemId, BalanceOf<T>, T::AccountId>,
				T::MaxExchangeItems,
			>,
			want_asks: BoundedVec<
				WantAsk<T::CollectionId, T::ItemId, BalanceOf<T>>,
				T::MaxExchangeItems,
			>,
			expiration_time: BlockNumberFor<T>,
			initial_amount: Option<BalanceOf<T>>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			Self::deposit_event(Event::ExchangeCreated { who, exchange: [0; 32] });
			Ok(())
		}

		/// Allows users to participate in the purchase of a suggestion inside an exchange.
		///
		/// The user commits to buying the item once the exchange is executed so the funds
		/// needed to purchase the item + fees are locked from the user's Balance.
		#[pallet::call_index(6)]
		#[pallet::weight({0})]
		pub fn fill_suggestion(
			origin: OriginFor<T>,
			exchange: HashId,
			collection: T::CollectionId,
			item: T::ItemId,
			fee: Permill,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			Self::deposit_event(Event::SuggestionFilled { who, exchange, collection, item });
			Ok(())
		}

		/// Trade suggested items with bidders that filled the suggestions and purchase the wantAsks
		/// - Bidders receive the unlocked item
		/// - Exchanger receives funds from sold items
		/// - After selling the items, the wantAsks are purchased from the marketplace using the generate profit
		/// - All the charged fees from bidders, sellers and exchanger go to the PayoutAddress
		///
		/// Only callable by exchange creator and when all suggestions have been filled.
		#[pallet::call_index(7)]
		#[pallet::weight({0})]
		pub fn execute_exchange(
			origin: OriginFor<T>,
			exec_suggestion: BoundedVec<
				ExecSuggestion<BoundedVec<u8, T::NonceStringLimit>>,
				T::MaxExchangeItems,
			>,
			exec_want_ask: BoundedVec<
				ExecWantAsk<BoundedVec<u8, T::NonceStringLimit>>,
				T::MaxExchangeItems,
			>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			Self::deposit_event(Event::ExchangeExecuted([0; 32]));
			Ok(())
		}

		/// Cancel an already filled suggestion. The suggestion must be part of an unclosed exchange
		///
		/// Only callable by user that filled the suggestion.
		///
		/// Unlocks amount of balance that the user locked for the purchase of the item.
		#[pallet::call_index(8)]
		#[pallet::weight({0})]
		pub fn cancel_suggestion(
			origin: OriginFor<T>,
			exchange: HashId,
			item_key: HashId,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			// Only used for id simulation for mock - TODO: Remove after mock
			let id = T::CollectionId::initial_value().ok_or(Error::<T>::InternalError)?;

			Self::deposit_event(Event::SuggestionCanceled {
				who,
				exchange,
				collection: id,
				item: id,
			});
			Ok(())
		}

		/// Cancels existing exchange marking it as closed. Only available for open exchanges
		///
		/// The suggested items and balance of the exchanger get unlocked
		/// Users that filled a suggestion get their balances unlocked
		///
		/// Only callable by exchange creator
		#[pallet::call_index(9)]
		#[pallet::weight({0})]
		pub fn cancel_exchange(origin: OriginFor<T>, exchange: HashId) -> DispatchResult {
			let who = ensure_signed(origin)?;

			Self::deposit_event(Event::ExchangeCanceled { who, exchange });
			Ok(())
		}
	}
	impl<T: Config> Pallet<T> {
		pub fn ensure_authority(who: &T::AccountId) -> Result<(), Error<T>> {
			match Authority::<T>::get().as_ref() {
				Some(who) => Ok(()),
				_ => Err(Error::<T>::NotAuthority),
			}
		}
		/* //TODO: create functionalities inside types.rs
		pub fn valid_match_exists_for() -> bool {
			true
		}
		pub fn calc_bid_payment() -> u128 {
			0u128
		}
		pub fn validate_exchange() -> bool {
			true
		} */
	}
}
