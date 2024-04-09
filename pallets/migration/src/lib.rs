#![cfg_attr(not(feature = "std"), no_std)]
#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::{
		pallet_prelude::*,
		traits::fungible::{Inspect, Mutate},
		PalletId,
	};
	use frame_system::{ensure_signed, pallet_prelude::*};
	use pallet_marketplace::{Ask, BalanceOf as MarketplaceBalanceOf};
	use pallet_nfts::NextCollectionId;

	use frame_support::{
		dispatch::GetDispatchInfo,
		traits::{
			nonfungibles_v2::Transfer, tokens::Preservation::Preserve, Incrementable,
			UnfilteredDispatchable,
		},
	};
	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config:
		frame_system::Config
		+ pallet_nfts::Config
		+ pallet_marketplace::Config
		+ pallet_timestamp::Config
	{
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		type RuntimeCall: Parameter
			+ UnfilteredDispatchable<RuntimeOrigin = Self::RuntimeOrigin>
			+ GetDispatchInfo;

		/// The fungible trait use for balance holds and transfers.
		type Currency: Inspect<Self::AccountId> + Mutate<Self::AccountId>;

		#[cfg(feature = "runtime-benchmarks")]
		/// A set of helper functions for benchmarking.
		type BenchmarkHelper: BenchmarkHelper<Self::CollectionId, Self::ItemId, Self::Moment>;
	}

	/// ID of this pallet.
	pub const PALLET_ID: PalletId = PalletId(*b"py/migra");

	pub type BalanceOf<T> =
		<<T as Config>::Currency as Inspect<<T as frame_system::Config>::AccountId>>::Balance;

	#[cfg(feature = "runtime-benchmarks")]
	pub trait BenchmarkHelper<CollectionId, ItemId, Moment> {
		/// Returns a collection id from a given integer.
		fn collection(id: u32) -> CollectionId;
		/// Returns an nft id from a given integer.
		fn item(id: u32) -> ItemId;
		fn timestamp(value: u64) -> Moment;
	}

	#[pallet::storage]
	#[pallet::getter(fn migrator)]
	pub type Migrator<T: Config> = StorageValue<_, T::AccountId, OptionQuery>;

	#[pallet::storage]
	#[pallet::getter(fn pot)]
	pub type Pot<T: Config> = StorageValue<_, T::AccountId, OptionQuery>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// The pallet's migrator was updated.
		MigratorUpdated(T::AccountId),
		/// The NextCollectionId was overwriten with a new value
		NextCollectionIdUpdated(T::CollectionId),
		/// An ask was created
		AskCreated {
			collection: T::CollectionId,
			item: T::ItemId,
			ask: Ask<T::AccountId, MarketplaceBalanceOf<T>, T::Moment>,
		},
		/// The pallet's Pot account was updated.
		PotUpdated(T::AccountId),
	}

	#[pallet::error]
	pub enum Error<T> {
		/// The caller is not the migrator account
		NotMigrator,
		///
		ItemNotFound,
		///
		InvalidExpiration,
		///
		PotAccountNotSet,
		/// Tried to store an account that is already set for this storage value.
		AccountAlreadySet,
		// Migrator is not set
		MigratorNotSet,
		/// Seller of ask is not the owner of the given item
		SellerNotItemOwner,
		///The account is already the owner of the item
		AlreadyOwner,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Sets migrator role, only callable by root origin
		#[pallet::call_index(0)]
		#[pallet::weight({0})]
		pub fn force_set_migrator(origin: OriginFor<T>, migrator: T::AccountId) -> DispatchResult {
			ensure_root(origin)?;

			ensure!(
				Migrator::<T>::get().as_ref() != Some(&migrator),
				Error::<T>::AccountAlreadySet
			);

			Migrator::<T>::put(migrator.clone());
			Self::deposit_event(Event::MigratorUpdated(migrator));
			Ok(())
		}

		#[pallet::call_index(1)]
		#[pallet::weight({0})]
		pub fn set_next_collection_id(
			origin: OriginFor<T>,
			collection_id: T::CollectionId,
		) -> DispatchResult {
			let _who = Self::ensure_migrator(origin)?;
			NextCollectionId::<T>::set(Some(collection_id.clone()));
			Self::deposit_event(Event::NextCollectionIdUpdated(collection_id));

			Ok(())
		}

		#[pallet::call_index(2)]
		#[pallet::weight({0})]
		pub fn create_ask(
			origin: OriginFor<T>,
			collection: T::CollectionId,
			item: T::ItemId,
			ask: Ask<T::AccountId, MarketplaceBalanceOf<T>, T::Moment>,
		) -> DispatchResult {
			let _who = Self::ensure_migrator(origin)?;

			let owner = pallet_nfts::Pallet::<T>::owner(collection.clone(), item.clone())
				.ok_or(Error::<T>::ItemNotFound)?;

			ensure!(owner == ask.seller, Error::<T>::SellerNotItemOwner);
			ensure!(
				ask.expiration > pallet_timestamp::Pallet::<T>::get(),
				Error::<T>::InvalidExpiration
			);

			pallet_marketplace::Asks::<T>::insert(&collection, &item, ask.clone());
			pallet_nfts::Pallet::<T>::disable_transfer(&collection, &item)?;
			Self::deposit_event(Event::AskCreated { collection, item, ask });

			Ok(())
		}

		#[pallet::call_index(3)]
		#[pallet::weight({0})]
		pub fn purge_item_data(
			origin: OriginFor<T>,
			collection: T::CollectionId,
			item: T::ItemId,
		) -> DispatchResult {
			todo!()
		}

		#[pallet::call_index(4)]
		#[pallet::weight({0})]
		pub fn set_pot_account(origin: OriginFor<T>, pot: T::AccountId) -> DispatchResult {
			let _who = Self::ensure_migrator(origin)?;

			ensure!(Pot::<T>::get().as_ref() != Some(&pot), Error::<T>::AccountAlreadySet);

			Pot::<T>::put(pot.clone());

			Self::deposit_event(Event::PotUpdated(pot));
			Ok(())
		}

		#[pallet::call_index(5)]
		#[pallet::weight({0})]
		pub fn send_funds_from_pot(
			origin: OriginFor<T>,
			recipient: T::AccountId,
			amount: BalanceOf<T>,
		) -> DispatchResult {
			let _who = Self::ensure_migrator(origin)?;

			let pot = Pot::<T>::get().ok_or(Error::<T>::PotAccountNotSet)?;
			<T as crate::Config>::Currency::transfer(&pot, &recipient, amount, Preserve)?;

			Ok(())
		}

		#[pallet::call_index(6)]
		#[pallet::weight({0})]
		pub fn set_item_owner(
			origin: OriginFor<T>,
			collection: T::CollectionId,
			item: T::ItemId,
			transfer_to: T::AccountId,
		) -> DispatchResult {
			Self::ensure_migrator(origin)?;

			let owner = pallet_nfts::Pallet::<T>::owner(collection.clone(), item.clone())
				.ok_or(Error::<T>::ItemNotFound)?;

			ensure!(owner != transfer_to, Error::<T>::AlreadyOwner);

			<pallet_nfts::Pallet<T> as Transfer<T::AccountId>>::transfer(
				&collection,
				&item,
				&transfer_to,
			)?;

			Ok(())
		}
	}
	impl<T: Config> Pallet<T> {
		pub fn ensure_migrator(origin: OriginFor<T>) -> Result<(), DispatchError> {
			let sender = ensure_signed(origin.clone())?;
			let migrator = Migrator::<T>::get().ok_or(Error::<T>::MigratorNotSet)?;
			ensure!(sender == migrator, Error::<T>::NotMigrator);
			Ok(())
		}

		#[cfg(test)]
		pub fn get_next_id() -> T::CollectionId {
			NextCollectionId::<T>::get()
				.or(T::CollectionId::initial_value())
				.expect("Failed to get next collection ID")
		}
	}
}

sp_core::generate_feature_enabled_macro!(runtime_benchmarks_enabled, feature = "runtime-benchmarks", $);