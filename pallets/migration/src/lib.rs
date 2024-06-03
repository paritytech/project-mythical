#![cfg_attr(not(feature = "std"), no_std)]
#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

pub mod weights;
pub use weights::*;

use parity_scale_codec::Codec;

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::{
		dispatch::GetDispatchInfo,
		traits::{
			nonfungibles_v2::Transfer, tokens::Preservation::Preserve, Currency,
			UnfilteredDispatchable,
		},
		PalletId,
	};
	use frame_support::{
		pallet_prelude::*,
		traits::{
			fungible::{Inspect, Mutate},
			Incrementable, SortedMembers,
		},
	};

	use frame_system::{ensure_signed, pallet_prelude::*};
	use pallet_marketplace::{Ask, BalanceOf as MarketplaceBalanceOf};
	use pallet_nfts::{CollectionConfig, ItemConfig, NextCollectionId, WeightInfo as NftWeight};
	use pallet_nfts::{CollectionConfigOf, ItemId};
	use sp_runtime::traits::{AccountIdConversion, StaticLookup};
	use sp_std::{vec, vec::Vec};

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

		/// Account Identifier from which the internal pot is generated.
		#[pallet::constant]
		type PotId: Get<PalletId>;

		/// Type representing the weight of this pallet
		type WeightInfo: WeightInfo;
	}

	pub type BalanceOf<T> =
		<<T as Config>::Currency as Inspect<<T as frame_system::Config>::AccountId>>::Balance;

	pub type NftBalanceOf<T> = <<T as pallet_nfts::Config>::Currency as Currency<
		<T as frame_system::Config>::AccountId,
	>>::Balance;

	type AccountIdLookupOf<T> = <<T as frame_system::Config>::Lookup as StaticLookup>::Source;

	#[pallet::storage]
	#[pallet::getter(fn migrator)]
	pub type Migrator<T: Config> = StorageValue<_, T::AccountId, OptionQuery>;
	pub struct MigratorProvider<T: crate::Config>(sp_std::marker::PhantomData<T>);

	impl<T: crate::Config> SortedMembers<T::AccountId> for MigratorProvider<T> {
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
		/// The NextCollectionId was overwriten with a new value
		NextCollectionIdUpdated(T::CollectionId),
		/// An ask was created
		AskCreated {
			collection: T::CollectionId,
			item: ItemId,
			ask: Ask<T::AccountId, MarketplaceBalanceOf<T>, T::Moment, T::AccountId>,
		},
		/// Serial mint collection config was enabled for the given collection
		SerialMintEnabled(T::CollectionId),
	}

	#[pallet::error]
	pub enum Error<T> {
		/// The caller is not the migrator account.
		NotMigrator,
		/// The item with the given collectionId and itemId was not found.
		ItemNotFound,
		/// Expiration below current timestamp.
		InvalidExpiration,
		/// Tried to store an account that is already set for this storage value.
		AccountAlreadySet,
		// Migrator is not set.
		MigratorNotSet,
		/// Seller of ask is not the owner of the given item.
		SellerNotItemOwner,
		/// The account is already the owner of the item.
		AlreadyOwner,
		/// The collection with the provided Id was not found.
		CollectionNotFound,
		/// Serial Mint config is already enabled for the given collection.
		SerialMintAlreadyEnabled,
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

		/// Sets the NextCollectionId on pallet-nfts, to be used as the CollectionIdwhen the next collection is created.
		///
		/// Only the migrator origin can execute this function. Migrator will not be charged fees for executing the extrinsic
		///
		/// Parameters:
		/// - `collectin_id`: Id no be set as NextCollectinId.
		///
		/// Emits NextCollectionIdUpdated when successful.
		///
		/// Weight: `WeightInfo::set_next_collection_id` (defined in the `Config` trait).
		#[pallet::call_index(1)]
		#[pallet::weight(<T as Config>::WeightInfo::set_next_collection_id())]
		pub fn set_next_collection_id(
			origin: OriginFor<T>,
			collection_id: T::CollectionId,
		) -> DispatchResultWithPostInfo {
			let _who = Self::ensure_migrator(origin)?;
			NextCollectionId::<T>::set(Some(collection_id.clone()));
			Self::deposit_event(Event::NextCollectionIdUpdated(collection_id));

			Ok(Pays::No.into())
		}

		/// Creates an Ask inside the Marketplace pallet's storage
		///
		/// Only the migrator origin can execute this function. Migrator will not be charged fees for executing the extrinsic
		///
		/// Parameters:
		/// - `collection`: Id of the collection for the item.
		/// - `item`: Id of the item.
		/// - `ask`: Marketplace ask to be created
		///
		/// Emits AskCreated when successful.
		///
		/// Weight: `WeightInfo::create_ask` (defined in the `Config` trait).
		#[pallet::call_index(2)]
		#[pallet::weight(<T as Config>::WeightInfo::create_ask())]
		pub fn create_ask(
			origin: OriginFor<T>,
			collection: T::CollectionId,
			item: ItemId,
			ask: Ask<T::AccountId, MarketplaceBalanceOf<T>, T::Moment, T::AccountId>,
		) -> DispatchResultWithPostInfo {
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

			Ok(Pays::No.into())
		}

		/// Transfer funds to a recipient account from the pot account.
		///
		/// Only the migrator origin can execute this function. Migrator will not be charged fees for executing the extrinsic
		///
		/// Parameters:
		/// - `recipient`: The account ID that will receive the funds.
		/// - `amount`: Amount of funds to be transfered to the recipient
		///
		/// Emits `Transfer` event upon successful execution.
		///
		/// Weight: `WeightInfo::send_funds_from_pot` (defined in the `Config` trait).
		#[pallet::call_index(3)]
		#[pallet::weight(<T as Config>::WeightInfo::send_funds_from_pot())]
		pub fn send_funds_from_pot(
			origin: OriginFor<T>,
			recipient: T::AccountId,
			amount: BalanceOf<T>,
		) -> DispatchResultWithPostInfo {
			let _who = Self::ensure_migrator(origin)?;

			let pot = Self::pot_account_id();
			<T as crate::Config>::Currency::transfer(&pot, &recipient, amount, Preserve)?;

			Ok(Pays::No.into())
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
		#[pallet::call_index(4)]
		#[pallet::weight(<T as Config>::WeightInfo::set_item_owner())]
		pub fn set_item_owner(
			origin: OriginFor<T>,
			collection: T::CollectionId,
			item: ItemId,
			transfer_to: T::AccountId,
		) -> DispatchResultWithPostInfo {
			Self::ensure_migrator(origin)?;

			let owner = pallet_nfts::Pallet::<T>::owner(collection.clone(), item.clone())
				.ok_or(Error::<T>::ItemNotFound)?;

			ensure!(owner != transfer_to, Error::<T>::AlreadyOwner);

			<pallet_nfts::Pallet<T> as Transfer<T::AccountId>>::transfer(
				&collection,
				&item,
				&transfer_to,
			)?;

			Ok(Pays::No.into())
		}

		/// Dispatches a call to pallet-nfts::force_create.
		///
		/// Only the migrator origin can execute this function. Migrator will not be charged fees for executing the extrinsic
		///
		/// Parameters:
		/// - `owner`: The owner of this collection of items. The owner has full superuser
		///   permissions over this item, but may later change and configure the permissions using
		///   `transfer_ownership` and `set_team`.
		///
		/// Emits `ForceCreated` event when successful.
		///
		/// Weight: `WeightInfo::force_create` (defined in the `Config` trait).
		#[pallet::call_index(5)]
		#[pallet::weight(<T as pallet_nfts::Config>::WeightInfo::force_create())]
		pub fn force_create(
			origin: OriginFor<T>,
			owner: AccountIdLookupOf<T>,
			config: CollectionConfig<NftBalanceOf<T>, BlockNumberFor<T>, T::CollectionId>,
		) -> DispatchResultWithPostInfo {
			Self::ensure_migrator(origin.clone())?;

			pallet_nfts::Pallet::<T>::force_create(origin, owner, config)?;

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
		#[pallet::call_index(6)]
		#[pallet::weight(<T as pallet_nfts::Config>::WeightInfo::set_team())]
		pub fn set_team(
			origin: OriginFor<T>,
			collection: T::CollectionId,
			issuer: Option<AccountIdLookupOf<T>>,
			admin: Option<AccountIdLookupOf<T>>,
			freezer: Option<AccountIdLookupOf<T>>,
		) -> DispatchResultWithPostInfo {
			Self::ensure_migrator(origin.clone())?;

			pallet_nfts::Pallet::<T>::set_team(origin, collection, issuer, admin, freezer)?;

			Ok(Pays::No.into())
		}

		/// Dispatches a call to pallet-nfts::set_collection_metadata.
		///
		/// Only the migrator origin can execute this function. Migrator will not be charged fees for executing the extrinsic
		///
		/// Parameters:
		/// - `collection`: The identifier of the item whose metadata to update.
		/// - `data`: The general information of this item. Limited in length by `StringLimit`.
		///
		/// Emits `CollectionMetadataSet`.
		///
		/// Weight: `WeightInfo::set_collection_metadata` (defined in the `Config` trait).
		#[pallet::call_index(7)]
		#[pallet::weight(<T as pallet_nfts::Config>::WeightInfo::set_collection_metadata())]
		pub fn set_collection_metadata(
			origin: OriginFor<T>,
			collection: T::CollectionId,
			data: BoundedVec<u8, T::StringLimit>,
		) -> DispatchResultWithPostInfo {
			Self::ensure_migrator(origin.clone())?;

			pallet_nfts::Pallet::<T>::set_collection_metadata(origin, collection, data)?;

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
		#[pallet::call_index(8)]
		#[pallet::weight(<T as pallet_nfts::Config>::WeightInfo::force_mint())]
		pub fn force_mint(
			origin: OriginFor<T>,
			collection: T::CollectionId,
			item: ItemId,
			mint_to: AccountIdLookupOf<T>,
			item_config: ItemConfig,
		) -> DispatchResultWithPostInfo {
			Self::ensure_migrator(origin.clone())?;

			pallet_nfts::Pallet::<T>::force_mint(
				origin,
				collection,
				Some(item),
				mint_to,
				item_config,
			)?;

			Ok(Pays::No.into())
		}

		#[pallet::call_index(9)]
		#[pallet::weight(<T as pallet_nfts::Config>::WeightInfo::force_mint())]
		pub fn enable_serial_mint(
			origin: OriginFor<T>,
			collection: T::CollectionId,
		) -> DispatchResultWithPostInfo {
			Self::ensure_migrator(origin.clone())?;

			let mut config =
				CollectionConfigOf::<T>::get(collection).ok_or(Error::<T>::CollectionNotFound)?;
			ensure!(!config.mint_settings.serial_mint, Error::<T>::SerialMintAlreadyEnabled);

			config.mint_settings.serial_mint = true;
			CollectionConfigOf::<T>::insert(collection, config);

			Self::deposit_event(Event::SerialMintEnabled(collection));

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

		/// Get a unique, inaccessible account ID from the `PotId`.
		pub fn pot_account_id() -> T::AccountId {
			T::PotId::get().into_account_truncating()
		}

		pub fn get_next_id() -> T::CollectionId {
			NextCollectionId::<T>::get()
				.or(T::CollectionId::initial_value())
				.expect("Failed to get next collection ID")
		}
	}
}

sp_core::generate_feature_enabled_macro!(runtime_benchmarks_enabled, feature = "runtime-benchmarks", $);

sp_api::decl_runtime_apis! {
	/// This runtime api allows to query the migration pot address.
	pub trait MigrationApi<AccountId>
	where AccountId: Codec
	{
		/// Queries the pot account.
		fn pot_account_id() -> AccountId;
	}
}
