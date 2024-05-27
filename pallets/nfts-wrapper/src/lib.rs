#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use frame_support::pallet_prelude::*;
	use frame_support::traits::Currency;
	use frame_support::traits::Incrementable;
	use frame_system::pallet_prelude::*;
	use pallet_nfts::{
		AttributeNamespace, CancelAttributesApprovalWitness, CollectionSettings, DestroyWitness,
		ItemConfig, ItemTip, MintSettings, MintWitness, PreSignedAttributes, PreSignedMint,
		PriceWithDirection,
	};
	use pallet_nfts::{CollectionConfig, WeightInfo};
	use sp_runtime::traits::{AtLeast32BitUnsigned, StaticLookup};
	use sp_runtime::traits::{One, Zero};
	use sp_runtime::Saturating;
	use sp_std::prelude::*;

	use super::*;

	type BalanceOf<T, I = ()> = <<T as pallet_nfts::Config<I>>::Currency as Currency<
		<T as frame_system::Config>::AccountId,
	>>::Balance;
	type DepositBalanceOf<T, I = ()> = <<T as pallet_nfts::Config<I>>::Currency as Currency<
		<T as frame_system::Config>::AccountId,
	>>::Balance;
	type CollectionConfigFor<T, I = ()> = CollectionConfig<
		BalanceOf<T>,
		BlockNumberFor<T>,
		<T as pallet_nfts::Config<I>>::CollectionId,
	>;
	type PreSignedMintOf<T, I = ()> = PreSignedMint<
		<T as pallet_nfts::Config<I>>::CollectionId,
		<T as pallet_nfts::Config<I>>::ItemId,
		<T as frame_system::Config>::AccountId,
		BlockNumberFor<T>,
		BalanceOf<T, I>,
	>;
	type PreSignedAttributesOf<T, I = ()> = PreSignedAttributes<
		<T as pallet_nfts::Config<I>>::CollectionId,
		<T as pallet_nfts::Config<I>>::ItemId,
		<T as frame_system::Config>::AccountId,
		BlockNumberFor<T>,
	>;
	type ItemTipOf<T, I = ()> = ItemTip<
		<T as pallet_nfts::Config<I>>::CollectionId,
		<T as pallet_nfts::Config<I>>::ItemId,
		<T as frame_system::Config>::AccountId,
		BalanceOf<T, I>,
	>;
	type ItemPrice<T, I = ()> = BalanceOf<T, I>;
	type AccountIdLookupOf<T> = <<T as frame_system::Config>::Lookup as StaticLookup>::Source;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_nfts::Config {
		/// Similar to the original ItemId in pallet-nfts, but restricting the type to a numeric value.
		type NumericItemId: Member
			+ Parameter
			+ MaxEncodedLen
			+ Copy
			+ AtLeast32BitUnsigned
			+ Default
			+ TryInto<<Self as pallet_nfts::Config>::ItemId>;
	}

	#[pallet::error]
	pub enum Error<T> {
		/// The item ID cannot be converted to the correct type, or it is over the max supply.
		InvalidItemId,
		/// Mint type has to be `Serial` to be able to be incremented.
		InvalidMintType,
	}

	/// Defines how items should be minted.
	#[derive(
		Default, Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen,
	)]
	pub enum MintType {
		/// Minting is done one by one, starting at zero.
		#[default]
		Serial,
		/// Minting is done arbitrarily.
		Random,
	}

	/// Extra information about a collection.
	#[derive(
		Default, Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen,
	)]
	pub struct ExtraCollectionDetails<ItemId> {
		/// Next collection's item ID, if serial minting is enabled.
		pub next_item_id: Option<ItemId>,
		/// The minting type.
		pub mint_type: MintType,
	}

	impl<ItemId> ExtraCollectionDetails<ItemId>
	where
		ItemId: Zero,
	{
		/// Creates a new instance of `ExtraCollectionDetails` based in the minting type.
		pub fn new(mint_type: MintType) -> Self {
			let next_item_id = match mint_type {
				MintType::Serial => Some(Zero::zero()),
				MintType::Random => None,
			};
			Self { next_item_id, mint_type }
		}
	}

	/// Stores the extra data for a collection.
	#[pallet::storage]
	pub type ExtraCollectionData<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		T::CollectionId,
		ExtraCollectionDetails<T::NumericItemId>,
		ValueQuery,
	>;

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Issue a new collection of non-fungible items from a public origin.
		#[pallet::call_index(0)]
		#[pallet::weight(<T as pallet_nfts::Config>::WeightInfo::create())]
		pub fn create(
			origin: OriginFor<T>,
			admin: AccountIdLookupOf<T>,
			config: CollectionConfigFor<T>,
			mint_type: MintType,
		) -> DispatchResult {
			if mint_type == MintType::Random {
				ensure!(config.max_supply.is_some(), Error::<T>::InvalidMintType);
			}
			let collection = pallet_nfts::NextCollectionId::<T>::get()
				.or(T::CollectionId::initial_value())
				.ok_or(pallet_nfts::Error::<T>::UnknownCollection)?;
			ExtraCollectionData::<T>::insert(&collection, ExtraCollectionDetails::new(mint_type));
			pallet_nfts::Pallet::<T>::create(origin, admin, config)
		}

		/// Issue a new collection of non-fungible items from a privileged origin.
		#[pallet::call_index(1)]
		#[pallet::weight(<T as pallet_nfts::Config>::WeightInfo::force_create())]
		pub fn force_create(
			origin: OriginFor<T>,
			owner: AccountIdLookupOf<T>,
			config: CollectionConfigFor<T>,
			mint_type: MintType,
		) -> DispatchResult {
			if mint_type == MintType::Random {
				ensure!(config.max_supply.is_some(), Error::<T>::InvalidMintType);
			}
			let collection = pallet_nfts::NextCollectionId::<T>::get()
				.or(T::CollectionId::initial_value())
				.ok_or(pallet_nfts::Error::<T>::UnknownCollection)?;
			ExtraCollectionData::<T>::insert(&collection, ExtraCollectionDetails::new(mint_type));
			pallet_nfts::Pallet::<T>::force_create(origin, owner, config)
		}

		/// Destroy a collection of fungible items.
		#[pallet::call_index(2)]
		#[pallet::weight(<T as pallet_nfts::Config>::WeightInfo::destroy(
		witness.item_metadatas,
		witness.item_configs,
		witness.attributes,
		))]
		pub fn destroy(
			origin: OriginFor<T>,
			collection: T::CollectionId,
			witness: DestroyWitness,
		) -> DispatchResultWithPostInfo {
			pallet_nfts::Pallet::<T>::destroy(origin, collection, witness)
		}

		/// Mint an item of a particular collection in serial mode.
		#[pallet::call_index(3)]
		#[pallet::weight(<T as pallet_nfts::Config>::WeightInfo::mint())]
		pub fn mint_serial(
			origin: OriginFor<T>,
			collection: T::CollectionId,
			mint_to: AccountIdLookupOf<T>,
			witness_data: Option<MintWitness<T::ItemId, DepositBalanceOf<T>>>,
		) -> DispatchResult {
			let extra = ExtraCollectionData::<T>::get(&collection);
			ensure!(extra.mint_type == MintType::Serial, Error::<T>::InvalidMintType);

			let final_item_id = Self::increment_collection_id(&collection)?;
			let max_supply = Self::get_collection_max_supply(&collection);
			ensure!(final_item_id <= max_supply.into(), Error::<T>::InvalidItemId);

			pallet_nfts::Pallet::<T>::mint(
				origin,
				collection,
				final_item_id.try_into().map_err(|_| Error::<T>::InvalidItemId)?,
				mint_to,
				witness_data,
			)
		}

		/// Mint an item of a particular collection in random mode.
		#[pallet::call_index(39)]
		#[pallet::weight(<T as pallet_nfts::Config>::WeightInfo::mint())]
		pub fn mint_any(
			origin: OriginFor<T>,
			collection: T::CollectionId,
			item: T::NumericItemId,
			mint_to: AccountIdLookupOf<T>,
			witness_data: Option<MintWitness<T::ItemId, DepositBalanceOf<T>>>,
		) -> DispatchResult {
			let extra = ExtraCollectionData::<T>::get(&collection);
			ensure!(extra.mint_type == MintType::Random, Error::<T>::InvalidMintType);

			let max_supply = Self::get_collection_max_supply(&collection);
			ensure!(item <= max_supply.into(), Error::<T>::InvalidItemId);

			pallet_nfts::Pallet::<T>::mint(
				origin,
				collection,
				item.try_into().map_err(|_| Error::<T>::InvalidItemId)?,
				mint_to,
				witness_data,
			)
		}

		/// Mint an item of a particular collection in serial mode from a privileged origin.
		#[pallet::call_index(4)]
		#[pallet::weight(<T as pallet_nfts::Config>::WeightInfo::force_mint())]
		pub fn force_mint_serial(
			origin: OriginFor<T>,
			collection: T::CollectionId,
			mint_to: AccountIdLookupOf<T>,
			item_config: ItemConfig,
		) -> DispatchResult {
			let extra = ExtraCollectionData::<T>::get(&collection);
			ensure!(extra.mint_type == MintType::Serial, Error::<T>::InvalidMintType);

			let final_item_id = Self::increment_collection_id(&collection)?;
			let max_supply = Self::get_collection_max_supply(&collection);
			ensure!(final_item_id <= max_supply.into(), Error::<T>::InvalidItemId);

			pallet_nfts::Pallet::<T>::force_mint(
				origin,
				collection,
				final_item_id.try_into().map_err(|_| Error::<T>::InvalidItemId)?,
				mint_to,
				item_config,
			)
		}

		/// Mint an item of a particular collection in random mode from a privileged origin.
		#[pallet::call_index(40)]
		#[pallet::weight(<T as pallet_nfts::Config>::WeightInfo::force_mint())]
		pub fn force_mint_any(
			origin: OriginFor<T>,
			collection: T::CollectionId,
			item: T::NumericItemId,
			mint_to: AccountIdLookupOf<T>,
			item_config: ItemConfig,
		) -> DispatchResult {
			let extra = ExtraCollectionData::<T>::get(&collection);
			ensure!(extra.mint_type == MintType::Random, Error::<T>::InvalidMintType);

			let max_supply = Self::get_collection_max_supply(&collection);
			ensure!(item <= max_supply.into(), Error::<T>::InvalidItemId);

			pallet_nfts::Pallet::<T>::force_mint(
				origin,
				collection,
				item.try_into().map_err(|_| Error::<T>::InvalidItemId)?,
				mint_to,
				item_config,
			)
		}

		/// Destroy a single item.
		#[pallet::call_index(5)]
		#[pallet::weight(<T as pallet_nfts::Config>::WeightInfo::burn())]
		pub fn burn(
			origin: OriginFor<T>,
			collection: T::CollectionId,
			item: T::ItemId,
		) -> DispatchResult {
			pallet_nfts::Pallet::<T>::burn(origin, collection, item)
		}

		/// Move an item from the sender account to another.
		#[pallet::call_index(6)]
		#[pallet::weight(<T as pallet_nfts::Config>::WeightInfo::transfer())]
		pub fn transfer(
			origin: OriginFor<T>,
			collection: T::CollectionId,
			item: T::ItemId,
			dest: AccountIdLookupOf<T>,
		) -> DispatchResult {
			pallet_nfts::Pallet::<T>::transfer(origin, collection, item, dest)
		}

		/// Re-evaluate the deposits on some items.
		#[pallet::call_index(7)]
		#[pallet::weight(<T as pallet_nfts::Config>::WeightInfo::redeposit(items.len() as u32))]
		pub fn redeposit(
			origin: OriginFor<T>,
			collection: T::CollectionId,
			items: Vec<T::ItemId>,
		) -> DispatchResult {
			pallet_nfts::Pallet::<T>::redeposit(origin, collection, items)
		}

		/// Disallow further unprivileged transfer of an item.
		#[pallet::call_index(8)]
		#[pallet::weight(<T as pallet_nfts::Config>::WeightInfo::lock_item_transfer())]
		pub fn lock_item_transfer(
			origin: OriginFor<T>,
			collection: T::CollectionId,
			item: T::ItemId,
		) -> DispatchResult {
			pallet_nfts::Pallet::<T>::lock_item_transfer(origin, collection, item)
		}

		/// Re-allow unprivileged transfer of an item.
		#[pallet::call_index(9)]
		#[pallet::weight(<T as pallet_nfts::Config>::WeightInfo::unlock_item_transfer())]
		pub fn unlock_item_transfer(
			origin: OriginFor<T>,
			collection: T::CollectionId,
			item: T::ItemId,
		) -> DispatchResult {
			pallet_nfts::Pallet::<T>::unlock_item_transfer(origin, collection, item)
		}

		/// Disallows specified settings for the whole collection.
		#[pallet::call_index(10)]
		#[pallet::weight(<T as pallet_nfts::Config>::WeightInfo::lock_collection())]
		pub fn lock_collection(
			origin: OriginFor<T>,
			collection: T::CollectionId,
			lock_settings: CollectionSettings,
		) -> DispatchResult {
			pallet_nfts::Pallet::<T>::lock_collection(origin, collection, lock_settings)
		}

		/// Change the Owner of a collection.
		#[pallet::call_index(11)]
		#[pallet::weight(<T as pallet_nfts::Config>::WeightInfo::transfer_ownership())]
		pub fn transfer_ownership(
			origin: OriginFor<T>,
			collection: T::CollectionId,
			new_owner: AccountIdLookupOf<T>,
		) -> DispatchResult {
			pallet_nfts::Pallet::<T>::transfer_ownership(origin, collection, new_owner)
		}

		/// Change the Issuer, Admin and Freezer of a collection.
		#[pallet::call_index(12)]
		#[pallet::weight(<T as pallet_nfts::Config>::WeightInfo::set_team())]
		pub fn set_team(
			origin: OriginFor<T>,
			collection: T::CollectionId,
			issuer: Option<AccountIdLookupOf<T>>,
			admin: Option<AccountIdLookupOf<T>>,
			freezer: Option<AccountIdLookupOf<T>>,
		) -> DispatchResult {
			pallet_nfts::Pallet::<T>::set_team(origin, collection, issuer, admin, freezer)
		}

		/// Change the Owner of a collection.
		#[pallet::call_index(13)]
		#[pallet::weight(<T as pallet_nfts::Config>::WeightInfo::force_collection_owner())]
		pub fn force_collection_owner(
			origin: OriginFor<T>,
			collection: T::CollectionId,
			owner: AccountIdLookupOf<T>,
		) -> DispatchResult {
			pallet_nfts::Pallet::<T>::force_collection_owner(origin, collection, owner)
		}

		/// Change the config of a collection.
		#[pallet::call_index(14)]
		#[pallet::weight(<T as pallet_nfts::Config>::WeightInfo::force_collection_config())]
		pub fn force_collection_config(
			origin: OriginFor<T>,
			collection: T::CollectionId,
			config: CollectionConfigFor<T>,
		) -> DispatchResult {
			pallet_nfts::Pallet::<T>::force_collection_config(origin, collection, config)
		}

		/// Approve an item to be transferred by a delegated third-party account.
		#[pallet::call_index(15)]
		#[pallet::weight(<T as pallet_nfts::Config>::WeightInfo::approve_transfer())]
		pub fn approve_transfer(
			origin: OriginFor<T>,
			collection: T::CollectionId,
			item: T::ItemId,
			delegate: AccountIdLookupOf<T>,
			maybe_deadline: Option<BlockNumberFor<T>>,
		) -> DispatchResult {
			pallet_nfts::Pallet::<T>::approve_transfer(
				origin,
				collection,
				item,
				delegate,
				maybe_deadline,
			)
		}

		/// Cancel one of the transfer approvals for a specific item.
		#[pallet::call_index(16)]
		#[pallet::weight(<T as pallet_nfts::Config>::WeightInfo::cancel_approval())]
		pub fn cancel_approval(
			origin: OriginFor<T>,
			collection: T::CollectionId,
			item: T::ItemId,
			delegate: AccountIdLookupOf<T>,
		) -> DispatchResult {
			pallet_nfts::Pallet::<T>::cancel_approval(origin, collection, item, delegate)
		}

		/// Cancel all the approvals of a specific item.
		#[pallet::call_index(17)]
		#[pallet::weight(<T as pallet_nfts::Config>::WeightInfo::clear_all_transfer_approvals())]
		pub fn clear_all_transfer_approvals(
			origin: OriginFor<T>,
			collection: T::CollectionId,
			item: T::ItemId,
		) -> DispatchResult {
			pallet_nfts::Pallet::<T>::clear_all_transfer_approvals(origin, collection, item)
		}

		/// Disallows changing the metadata or attributes of the item.
		#[pallet::call_index(18)]
		#[pallet::weight(<T as pallet_nfts::Config>::WeightInfo::lock_item_properties())]
		pub fn lock_item_properties(
			origin: OriginFor<T>,
			collection: T::CollectionId,
			item: T::ItemId,
			lock_metadata: bool,
			lock_attributes: bool,
		) -> DispatchResult {
			pallet_nfts::Pallet::<T>::lock_item_properties(
				origin,
				collection,
				item,
				lock_metadata,
				lock_attributes,
			)
		}

		/// Set an attribute for a collection or item.
		#[pallet::call_index(19)]
		#[pallet::weight(<T as pallet_nfts::Config>::WeightInfo::set_attribute())]
		pub fn set_attribute(
			origin: OriginFor<T>,
			collection: T::CollectionId,
			maybe_item: Option<T::ItemId>,
			namespace: AttributeNamespace<T::AccountId>,
			key: BoundedVec<u8, T::KeyLimit>,
			value: BoundedVec<u8, T::ValueLimit>,
		) -> DispatchResult {
			pallet_nfts::Pallet::<T>::set_attribute(
				origin, collection, maybe_item, namespace, key, value,
			)
		}

		/// Force-set an attribute for a collection or item.
		#[pallet::call_index(20)]
		#[pallet::weight(<T as pallet_nfts::Config>::WeightInfo::force_set_attribute())]
		pub fn force_set_attribute(
			origin: OriginFor<T>,
			set_as: Option<T::AccountId>,
			collection: T::CollectionId,
			maybe_item: Option<T::ItemId>,
			namespace: AttributeNamespace<T::AccountId>,
			key: BoundedVec<u8, T::KeyLimit>,
			value: BoundedVec<u8, T::ValueLimit>,
		) -> DispatchResult {
			pallet_nfts::Pallet::<T>::force_set_attribute(
				origin, set_as, collection, maybe_item, namespace, key, value,
			)
		}

		/// Clear an attribute for a collection or item.
		#[pallet::call_index(21)]
		#[pallet::weight(<T as pallet_nfts::Config>::WeightInfo::clear_attribute())]
		pub fn clear_attribute(
			origin: OriginFor<T>,
			collection: T::CollectionId,
			maybe_item: Option<T::ItemId>,
			namespace: AttributeNamespace<T::AccountId>,
			key: BoundedVec<u8, T::KeyLimit>,
		) -> DispatchResult {
			pallet_nfts::Pallet::<T>::clear_attribute(
				origin, collection, maybe_item, namespace, key,
			)
		}

		/// Approve item's attributes to be changed by a delegated third-party account.
		#[pallet::call_index(22)]
		#[pallet::weight(<T as pallet_nfts::Config>::WeightInfo::approve_item_attributes())]
		pub fn approve_item_attributes(
			origin: OriginFor<T>,
			collection: T::CollectionId,
			item: T::ItemId,
			delegate: AccountIdLookupOf<T>,
		) -> DispatchResult {
			pallet_nfts::Pallet::<T>::approve_item_attributes(origin, collection, item, delegate)
		}

		/// Cancel the previously provided approval to change item's attributes.
		#[pallet::call_index(23)]
		#[pallet::weight(<T as pallet_nfts::Config>::WeightInfo::cancel_item_attributes_approval(
		witness.account_attributes
		))]
		pub fn cancel_item_attributes_approval(
			origin: OriginFor<T>,
			collection: T::CollectionId,
			item: T::ItemId,
			delegate: AccountIdLookupOf<T>,
			witness: CancelAttributesApprovalWitness,
		) -> DispatchResult {
			pallet_nfts::Pallet::<T>::cancel_item_attributes_approval(
				origin, collection, item, delegate, witness,
			)
		}

		/// Set the metadata for an item.
		#[pallet::call_index(24)]
		#[pallet::weight(<T as pallet_nfts::Config>::WeightInfo::set_metadata())]
		pub fn set_metadata(
			origin: OriginFor<T>,
			collection: T::CollectionId,
			item: T::ItemId,
			data: BoundedVec<u8, T::StringLimit>,
		) -> DispatchResult {
			pallet_nfts::Pallet::<T>::set_metadata(origin, collection, item, data)
		}

		/// Clear the metadata for an item.
		#[pallet::call_index(25)]
		#[pallet::weight(<T as pallet_nfts::Config>::WeightInfo::clear_metadata())]
		pub fn clear_metadata(
			origin: OriginFor<T>,
			collection: T::CollectionId,
			item: T::ItemId,
		) -> DispatchResult {
			pallet_nfts::Pallet::<T>::clear_metadata(origin, collection, item)
		}

		/// Set the metadata for a collection.
		#[pallet::call_index(26)]
		#[pallet::weight(<T as pallet_nfts::Config>::WeightInfo::set_collection_metadata())]
		pub fn set_collection_metadata(
			origin: OriginFor<T>,
			collection: T::CollectionId,
			data: BoundedVec<u8, T::StringLimit>,
		) -> DispatchResult {
			pallet_nfts::Pallet::<T>::set_collection_metadata(origin, collection, data)
		}

		/// Clear the metadata for a collection.
		#[pallet::call_index(27)]
		#[pallet::weight(<T as pallet_nfts::Config>::WeightInfo::clear_collection_metadata())]
		pub fn clear_collection_metadata(
			origin: OriginFor<T>,
			collection: T::CollectionId,
		) -> DispatchResult {
			pallet_nfts::Pallet::<T>::clear_collection_metadata(origin, collection)
		}

		/// Set (or reset) the acceptance of ownership for a particular account.
		#[pallet::call_index(28)]
		#[pallet::weight(<T as pallet_nfts::Config>::WeightInfo::set_accept_ownership())]
		pub fn set_accept_ownership(
			origin: OriginFor<T>,
			maybe_collection: Option<T::CollectionId>,
		) -> DispatchResult {
			pallet_nfts::Pallet::<T>::set_accept_ownership(origin, maybe_collection)
		}

		/// Set the maximum number of items a collection could have.
		#[pallet::call_index(29)]
		#[pallet::weight(<T as pallet_nfts::Config>::WeightInfo::set_collection_max_supply())]
		pub fn set_collection_max_supply(
			origin: OriginFor<T>,
			collection: T::CollectionId,
			max_supply: u32,
		) -> DispatchResult {
			pallet_nfts::Pallet::<T>::set_collection_max_supply(origin, collection, max_supply)
		}

		/// Update mint settings.
		#[pallet::call_index(30)]
		#[pallet::weight(<T as pallet_nfts::Config>::WeightInfo::update_mint_settings())]
		pub fn update_mint_settings(
			origin: OriginFor<T>,
			collection: T::CollectionId,
			mint_settings: MintSettings<BalanceOf<T>, BlockNumberFor<T>, T::CollectionId>,
		) -> DispatchResult {
			pallet_nfts::Pallet::<T>::update_mint_settings(origin, collection, mint_settings)
		}

		/// Set (or reset) the price for an item.
		#[pallet::call_index(31)]
		#[pallet::weight(<T as pallet_nfts::Config>::WeightInfo::set_price())]
		pub fn set_price(
			origin: OriginFor<T>,
			collection: T::CollectionId,
			item: T::ItemId,
			price: Option<ItemPrice<T>>,
			whitelisted_buyer: Option<AccountIdLookupOf<T>>,
		) -> DispatchResult {
			pallet_nfts::Pallet::<T>::set_price(origin, collection, item, price, whitelisted_buyer)
		}

		/// Allows to buy an item if it's up for sale.
		#[pallet::call_index(32)]
		#[pallet::weight(<T as pallet_nfts::Config>::WeightInfo::buy_item())]
		pub fn buy_item(
			origin: OriginFor<T>,
			collection: T::CollectionId,
			item: T::ItemId,
			bid_price: ItemPrice<T>,
		) -> DispatchResult {
			pallet_nfts::Pallet::<T>::buy_item(origin, collection, item, bid_price)
		}

		/// Allows to pay the tips.
		#[pallet::call_index(33)]
		#[pallet::weight(<T as pallet_nfts::Config>::WeightInfo::pay_tips(tips.len() as u32))]
		pub fn pay_tips(
			origin: OriginFor<T>,
			tips: BoundedVec<ItemTipOf<T>, T::MaxTips>,
		) -> DispatchResult {
			pallet_nfts::Pallet::<T>::pay_tips(origin, tips)
		}

		/// Register a new atomic swap, declaring an intention to send an `item` in exchange for
		/// `desired_item` from origin to target on the current blockchain.
		#[pallet::call_index(34)]
		#[pallet::weight(<T as pallet_nfts::Config>::WeightInfo::create_swap())]
		pub fn create_swap(
			origin: OriginFor<T>,
			offered_collection: T::CollectionId,
			offered_item: T::ItemId,
			desired_collection: T::CollectionId,
			maybe_desired_item: Option<T::ItemId>,
			maybe_price: Option<PriceWithDirection<ItemPrice<T>>>,
			duration: BlockNumberFor<T>,
		) -> DispatchResult {
			pallet_nfts::Pallet::<T>::create_swap(
				origin,
				offered_collection,
				offered_item,
				desired_collection,
				maybe_desired_item,
				maybe_price,
				duration,
			)
		}

		/// Cancel an atomic swap.
		#[pallet::call_index(35)]
		#[pallet::weight(<T as pallet_nfts::Config>::WeightInfo::cancel_swap())]
		pub fn cancel_swap(
			origin: OriginFor<T>,
			offered_collection: T::CollectionId,
			offered_item: T::ItemId,
		) -> DispatchResult {
			pallet_nfts::Pallet::<T>::cancel_swap(origin, offered_collection, offered_item)
		}

		/// Claim an atomic swap.
		#[pallet::call_index(36)]
		#[pallet::weight(<T as pallet_nfts::Config>::WeightInfo::claim_swap())]
		pub fn claim_swap(
			origin: OriginFor<T>,
			send_collection: T::CollectionId,
			send_item: T::ItemId,
			receive_collection: T::CollectionId,
			receive_item: T::ItemId,
			witness_price: Option<PriceWithDirection<ItemPrice<T>>>,
		) -> DispatchResult {
			pallet_nfts::Pallet::<T>::claim_swap(
				origin,
				send_collection,
				send_item,
				receive_collection,
				receive_item,
				witness_price,
			)
		}

		/// Mint an item by providing the pre-signed approval.
		#[pallet::call_index(37)]
		#[pallet::weight(
		// TODO cannot access data.attributes
			<T as pallet_nfts::Config>::WeightInfo::mint_pre_signed(0)
		)]
		pub fn mint_pre_signed(
			origin: OriginFor<T>,
			mint_data: Box<PreSignedMintOf<T>>,
			signature: T::OffchainSignature,
			signer: T::AccountId,
		) -> DispatchResult {
			pallet_nfts::Pallet::<T>::mint_pre_signed(origin, mint_data, signature, signer)
		}

		/// Set attributes for an item by providing the pre-signed approval.
		#[pallet::call_index(38)]
		#[pallet::weight(
		// TODO cannot access data.attributes
			<T as pallet_nfts::Config>::WeightInfo::set_attributes_pre_signed(0)
		)]
		pub fn set_attributes_pre_signed(
			origin: OriginFor<T>,
			data: PreSignedAttributesOf<T>,
			signature: T::OffchainSignature,
			signer: T::AccountId,
		) -> DispatchResult {
			pallet_nfts::Pallet::<T>::set_attributes_pre_signed(origin, data, signature, signer)
		}
	}

	impl<T: Config> Pallet<T> {
		/// Increments the next item ID of a collection.
		///
		/// Returns the ID before being incremented.
		fn increment_collection_id(
			collection: &T::CollectionId,
		) -> Result<T::NumericItemId, DispatchError> {
			ExtraCollectionData::<T>::mutate(
				&collection,
				|extra| -> Result<T::NumericItemId, DispatchError> {
					ensure!(extra.mint_type == MintType::Serial, Error::<T>::InvalidMintType);
					let id = if let Some(next_item_id) = extra.next_item_id {
						let new_item_id = next_item_id.saturating_add(One::one());
						extra.next_item_id = Some(new_item_id);
						new_item_id
					} else {
						return Err(Error::<T>::InvalidMintType.into());
					};
					Ok(id)
				},
			)
		}

		/// Returns a collection's maximum supply.
		///
		/// If not present, returns u32::MAX.
		fn get_collection_max_supply(collection: &T::CollectionId) -> u32 {
			if let Some(collection_config) = pallet_nfts::CollectionConfigOf::<T>::get(&collection)
			{
				collection_config.max_supply.unwrap_or(u32::MAX)
			} else {
				u32::MAX
			}
		}
	}
}
