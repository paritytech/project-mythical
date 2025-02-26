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
		dispatch::GetDispatchInfo,
		traits::{nonfungibles_v2::Transfer, Currency, UnfilteredDispatchable},
	};
	use frame_support::{
		pallet_prelude::*,
		traits::{
			fungible::{Inspect, Mutate},
			SortedMembers,
		},
	};

	use frame_system::{ensure_signed, pallet_prelude::*};
	use pallet_dmarket::DmarketCollection;
	use pallet_nfts::ItemId;
	use pallet_nfts::{ItemConfig, WeightInfo as NftWeight};
	use sp_runtime::traits::StaticLookup;
	use sp_std::{vec, vec::Vec};

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config:
		frame_system::Config
		+ pallet_nfts::Config
		+ pallet_dmarket::Config
		+ pallet_timestamp::Config
	{
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		type RuntimeCall: Parameter
			+ UnfilteredDispatchable<RuntimeOrigin = Self::RuntimeOrigin>
			+ GetDispatchInfo;

		/// The fungible trait use for balance holds and transfers.
		type Currency: Inspect<Self::AccountId> + Mutate<Self::AccountId>;

		/// Type representing the weight of this pallet
		type WeightInfo: WeightInfo;
		#[cfg(feature = "runtime-benchmarks")]
		/// A set of helper functions for benchmarking.
		type BenchmarkHelper: BenchmarkHelper<Self::CollectionId, Self::Moment>;
	}

	pub type BalanceOf<T> =
		<<T as Config>::Currency as Inspect<<T as frame_system::Config>::AccountId>>::Balance;

	pub type NftBalanceOf<T> = <<T as pallet_nfts::Config>::Currency as Currency<
		<T as frame_system::Config>::AccountId,
	>>::Balance;

	#[cfg(feature = "runtime-benchmarks")]
	pub trait BenchmarkHelper<CollectionId, Moment> {
		/// Returns a collection id from a given integer.
		fn collection(id: u16) -> CollectionId;
		/// Returns a NFT id from a given integer.
		fn timestamp(value: u64) -> Moment;
	}

	type AccountIdLookupOf<T> = <<T as frame_system::Config>::Lookup as StaticLookup>::Source;

	#[pallet::storage]
	#[pallet::getter(fn migrator)]
	pub type Migrator<T: Config> = StorageValue<_, T::AccountId, OptionQuery>;
	pub struct MigratorProvider<T: crate::Config>(sp_std::marker::PhantomData<T>);

	impl<T: Config> SortedMembers<T::AccountId> for MigratorProvider<T> {
		fn sorted_members() -> Vec<T::AccountId> {
			if let Some(migrator) = Migrator::<T>::get() {
				return vec![migrator];
			}
			vec![]
		}

		fn contains(who: &T::AccountId) -> bool {
			if let Some(migrator) = Migrator::<T>::get() {
				return migrator == *who;
			}
			false
		}
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// The pallet's migrator was updated.
		MigratorUpdated(T::AccountId),
	}

	#[pallet::error]
	pub enum Error<T> {
		/// The caller is not the migrator account.
		NotMigrator,
		/// The item with the given collectionId and itemId was not found.
		ItemNotFound,
		/// Tried to store an account that is already set for this storage value.
		AccountAlreadySet,
		/// Migrator is not set.
		MigratorNotSet,
		/// The account is already the owner of the item.
		AlreadyOwner,
		/// The DmarketCollection is not configured.
		DmarketCollectionNotSet,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Sets the migrator role, granting rights to call this pallet's extrinsics.
		///
		/// Only the root origin can execute this function.
		///
		/// Parameters:
		/// - `migrator`: The account ID to be set as the pallet's migrator.
		///
		/// Emits MigratorUpdated when successful.
		///
		/// Weight: `WeightInfo::force_set_migrator` (defined in the `Config` trait).
		#[pallet::call_index(0)]
		#[pallet::weight(<T as Config>::WeightInfo::force_set_migrator())]
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

		/// Transfers a given Nft to an AccountId.
		///
		/// Only the migrator origin can execute this function. Migrator will not be charged fees for executing the extrinsic
		///
		/// Parameters:
		/// - `collection`: Id of the collection for the item.
		/// - `item`: Id of the item.
		/// - `transfer_to`: AccountId of the user that will receive the item
		///
		/// Emits `Transferred` event upon successful execution.
		///
		/// Weight: `WeightInfo::set_item_owner` (defined in the `Config` trait).
		#[pallet::call_index(1)]
		#[pallet::weight(<T as Config>::WeightInfo::set_item_owner())]
		pub fn set_item_owner(
			origin: OriginFor<T>,
			item: ItemId,
			transfer_to: T::AccountId,
		) -> DispatchResultWithPostInfo {
			Self::ensure_migrator(origin)?;
			let collection = Self::get_dmarket_collection()?;

			let owner = pallet_nfts::Pallet::<T>::owner(collection, item)
				.ok_or(Error::<T>::ItemNotFound)?;

			ensure!(owner != transfer_to, Error::<T>::AlreadyOwner);

			<pallet_nfts::Pallet<T> as Transfer<T::AccountId>>::transfer(
				&collection,
				&item,
				&transfer_to,
			)?;

			Ok(Pays::No.into())
		}

		/// Dispatches a call to pallet-nfts::set_team.
		///
		/// Only the migrator origin can execute this function. Migrator will not be charged fees for executing the extrinsic
		///
		/// Parameters:
		/// - `collection`: The collection whose team should be changed.
		/// - `issuer`: The new Issuer of this collection.
		/// - `admin`: The new Admin of this collection.
		/// - `freezer`: The new Freezer of this collection.
		///
		/// Emits `TeamChanged`.
		///
		/// Weight: `WeightInfo::set_team` (defined in the `Config` trait).
		#[pallet::call_index(2)]
		#[pallet::weight(<T as pallet_nfts::Config>::WeightInfo::set_team())]
		pub fn set_team(
			origin: OriginFor<T>,
			issuer: Option<AccountIdLookupOf<T>>,
			admin: Option<AccountIdLookupOf<T>>,
			freezer: Option<AccountIdLookupOf<T>>,
		) -> DispatchResultWithPostInfo {
			Self::ensure_migrator(origin.clone())?;
			let collection = Self::get_dmarket_collection()?;

			pallet_nfts::Pallet::<T>::set_team(origin, collection, issuer, admin, freezer)?;

			Ok(Pays::No.into())
		}

		/// Dispatches a call to pallet-nfts::force_mint.
		///
		/// Only the migrator origin can execute this function. Migrator will not be charged fees for executing the extrinsic
		///
		/// Parameters:
		/// - `collection`: The collection of the item to be minted.
		/// - `item`: An identifier of the new item.
		/// - `mint_to`: Account into which the item will be minted.
		/// - `item_config`: A config of the new item.
		///
		/// Emits `Issued` event when successful.
		///
		/// Weight: `WeightInfo::force_mint` (defined in the `Config` trait).
		#[pallet::call_index(3)]
		#[pallet::weight(<T as pallet_nfts::Config>::WeightInfo::force_mint().saturating_add(T::DbWeight::get().reads(2_u64)))]
		pub fn force_mint(
			origin: OriginFor<T>,
			item: ItemId,
			mint_to: AccountIdLookupOf<T>,
			item_config: ItemConfig,
		) -> DispatchResultWithPostInfo {
			Self::ensure_migrator(origin.clone())?;
			let collection = Self::get_dmarket_collection()?;

			pallet_nfts::Pallet::<T>::force_mint(
				origin,
				collection,
				Some(item),
				mint_to,
				item_config,
			)?;

			Ok(Pays::No.into())
		}
	}
	impl<T: Config> Pallet<T> {
		pub fn ensure_migrator(origin: OriginFor<T>) -> Result<(), DispatchError> {
			let sender = ensure_signed(origin.clone())?;
			let migrator = Migrator::<T>::get().ok_or(Error::<T>::MigratorNotSet)?;
			ensure!(sender == migrator, Error::<T>::NotMigrator);
			Ok(())
		}

		pub fn get_dmarket_collection() -> Result<T::CollectionId, DispatchError> {
			let dmarket_collection =
				DmarketCollection::<T>::get().ok_or(Error::<T>::DmarketCollectionNotSet)?;
			Ok(dmarket_collection)
		}
	}
}

sp_core::generate_feature_enabled_macro!(runtime_benchmarks_enabled, feature = "runtime-benchmarks", $);
