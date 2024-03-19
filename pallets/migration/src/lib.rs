#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::{pallet_prelude::*, traits::fungible::Inspect};
	use frame_system::{
		ensure_signed,
		pallet_prelude::*,
	};
    use pallet_marketplace::{Ask, Asks, BalanceOf};
    use pallet_nfts::NextCollectionId;

	use frame_support::{dispatch::GetDispatchInfo, traits::UnfilteredDispatchable};

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
	}

	#[pallet::storage]
	#[pallet::getter(fn migrator)]
	pub type Migrator<T: Config> = StorageValue<_, T::AccountId, OptionQuery>;

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
			ask: Ask<T::AccountId, BalanceOf<T>, T::Moment>,
		},
	}

	#[pallet::error]
	pub enum Error<T> {
		/// The caller is not the migrator account
		NotMigrator,
        ///
        ItemNotFound,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Sets migrator role, only callable by root origin
		#[pallet::call_index(0)]
		#[pallet::weight({0})]
		pub fn force_set_migrator(origin: OriginFor<T>, migrator: T::AccountId) -> DispatchResult {
			ensure_root(origin)?;

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
			let who = ensure_signed(origin)?;
			Self::ensure_migrator(&who)?;

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
			ask: Ask<T::AccountId, BalanceOf<T>, T::Moment>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			Self::ensure_migrator(&who)?;

            pallet_nfts::Pallet::<T>::owner(collection.clone(), item.clone())
					.ok_or(Error::<T>::ItemNotFound)?;

			pallet_marketplace::Asks::<T>::insert(
				collection.clone(),
				item.clone(),
				ask.clone(),
			);
			Self::deposit_event(Event::AskCreated { collection, item, ask });

			Ok(())
		}
	}
	impl<T: Config> Pallet<T> {
		pub fn ensure_migrator(who: &T::AccountId) -> Result<(), Error<T>> {
			match Migrator::<T>::get().as_ref() {
				Some(who) => Ok(()),
				_ => Err(Error::<T>::NotMigrator),
			}
		}
	}
}
