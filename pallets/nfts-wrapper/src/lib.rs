#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use frame_support::pallet_prelude::*;
	use frame_support::traits::Currency;
	use frame_system::pallet_prelude::*;
	use pallet_nfts::{
		AttributeNamespace, CancelAttributesApprovalWitness, CollectionSettings, DestroyWitness,
		ItemConfig, ItemTip, MintSettings, MintWitness, PreSignedAttributes, PreSignedMint,
		PriceWithDirection,
	};
	use pallet_nfts::{CollectionConfig, WeightInfo};
	use sp_runtime::traits::One;
	use sp_runtime::traits::{AtLeast32BitUnsigned, StaticLookup};
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
		InvalidItemId,
	}

	#[pallet::storage]
	pub type NextItemId<T: Config> =
		StorageMap<_, Blake2_128Concat, T::CollectionId, T::NumericItemId, ValueQuery>;

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::call_index(0)]
		#[pallet::weight(<T as pallet_nfts::Config>::WeightInfo::create())]
		pub fn create(
			origin: OriginFor<T>,
			admin: AccountIdLookupOf<T>,
			config: CollectionConfigFor<T>,
		) -> DispatchResult {
			pallet_nfts::Pallet::<T>::create(origin, admin, config)
		}

		#[pallet::call_index(1)]
		#[pallet::weight(<T as pallet_nfts::Config>::WeightInfo::force_create())]
		pub fn force_create(
			origin: OriginFor<T>,
			owner: AccountIdLookupOf<T>,
			config: CollectionConfigFor<T>,
		) -> DispatchResult {
			pallet_nfts::Pallet::<T>::force_create(origin, owner, config)
		}

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

		#[pallet::call_index(3)]
		#[pallet::weight(<T as pallet_nfts::Config>::WeightInfo::mint())]
		pub fn mint(
			origin: OriginFor<T>,
			collection: T::CollectionId,
			_item: T::ItemId,
			mint_to: AccountIdLookupOf<T>,
			witness_data: Option<MintWitness<T::ItemId, DepositBalanceOf<T>>>,
		) -> DispatchResult {
			let next_item_id = Self::increment_collection_id(&collection)
				.try_into()
				.map_err(|_| Error::<T>::InvalidItemId)?;
			pallet_nfts::Pallet::<T>::mint(origin, collection, next_item_id, mint_to, witness_data)
		}

		#[pallet::call_index(4)]
		#[pallet::weight(<T as pallet_nfts::Config>::WeightInfo::force_mint())]
		pub fn force_mint(
			origin: OriginFor<T>,
			collection: T::CollectionId,
			_item: T::ItemId,
			mint_to: AccountIdLookupOf<T>,
			item_config: ItemConfig,
		) -> DispatchResult {
			let next_item_id = Self::increment_collection_id(&collection)
				.try_into()
				.map_err(|_| Error::<T>::InvalidItemId)?;
			pallet_nfts::Pallet::<T>::force_mint(
				origin,
				collection,
				next_item_id,
				mint_to,
				item_config,
			)
		}

		#[pallet::call_index(5)]
		#[pallet::weight(<T as pallet_nfts::Config>::WeightInfo::burn())]
		pub fn burn(
			origin: OriginFor<T>,
			collection: T::CollectionId,
			item: T::ItemId,
		) -> DispatchResult {
			pallet_nfts::Pallet::<T>::burn(origin, collection, item)
		}

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

		#[pallet::call_index(7)]
		#[pallet::weight(<T as pallet_nfts::Config>::WeightInfo::redeposit(items.len() as u32))]
		pub fn redeposit(
			origin: OriginFor<T>,
			collection: T::CollectionId,
			items: Vec<T::ItemId>,
		) -> DispatchResult {
			pallet_nfts::Pallet::<T>::redeposit(origin, collection, items)
		}

		#[pallet::call_index(8)]
		#[pallet::weight(<T as pallet_nfts::Config>::WeightInfo::lock_item_transfer())]
		pub fn lock_item_transfer(
			origin: OriginFor<T>,
			collection: T::CollectionId,
			item: T::ItemId,
		) -> DispatchResult {
			pallet_nfts::Pallet::<T>::lock_item_transfer(origin, collection, item)
		}

		#[pallet::call_index(9)]
		#[pallet::weight(<T as pallet_nfts::Config>::WeightInfo::unlock_item_transfer())]
		pub fn unlock_item_transfer(
			origin: OriginFor<T>,
			collection: T::CollectionId,
			item: T::ItemId,
		) -> DispatchResult {
			pallet_nfts::Pallet::<T>::unlock_item_transfer(origin, collection, item)
		}

		#[pallet::call_index(10)]
		#[pallet::weight(<T as pallet_nfts::Config>::WeightInfo::lock_collection())]
		pub fn lock_collection(
			origin: OriginFor<T>,
			collection: T::CollectionId,
			lock_settings: CollectionSettings,
		) -> DispatchResult {
			pallet_nfts::Pallet::<T>::lock_collection(origin, collection, lock_settings)
		}

		#[pallet::call_index(11)]
		#[pallet::weight(<T as pallet_nfts::Config>::WeightInfo::transfer_ownership())]
		pub fn transfer_ownership(
			origin: OriginFor<T>,
			collection: T::CollectionId,
			new_owner: AccountIdLookupOf<T>,
		) -> DispatchResult {
			pallet_nfts::Pallet::<T>::transfer_ownership(origin, collection, new_owner)
		}

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

		#[pallet::call_index(13)]
		#[pallet::weight(<T as pallet_nfts::Config>::WeightInfo::force_collection_owner())]
		pub fn force_collection_owner(
			origin: OriginFor<T>,
			collection: T::CollectionId,
			owner: AccountIdLookupOf<T>,
		) -> DispatchResult {
			pallet_nfts::Pallet::<T>::force_collection_owner(origin, collection, owner)
		}

		#[pallet::call_index(14)]
		#[pallet::weight(<T as pallet_nfts::Config>::WeightInfo::force_collection_config())]
		pub fn force_collection_config(
			origin: OriginFor<T>,
			collection: T::CollectionId,
			config: CollectionConfigFor<T>,
		) -> DispatchResult {
			pallet_nfts::Pallet::<T>::force_collection_config(origin, collection, config)
		}

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

		#[pallet::call_index(17)]
		#[pallet::weight(<T as pallet_nfts::Config>::WeightInfo::clear_all_transfer_approvals())]
		pub fn clear_all_transfer_approvals(
			origin: OriginFor<T>,
			collection: T::CollectionId,
			item: T::ItemId,
		) -> DispatchResult {
			pallet_nfts::Pallet::<T>::clear_all_transfer_approvals(origin, collection, item)
		}

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

		#[pallet::call_index(25)]
		#[pallet::weight(<T as pallet_nfts::Config>::WeightInfo::clear_metadata())]
		pub fn clear_metadata(
			origin: OriginFor<T>,
			collection: T::CollectionId,
			item: T::ItemId,
		) -> DispatchResult {
			pallet_nfts::Pallet::<T>::clear_metadata(origin, collection, item)
		}

		#[pallet::call_index(26)]
		#[pallet::weight(<T as pallet_nfts::Config>::WeightInfo::set_collection_metadata())]
		pub fn set_collection_metadata(
			origin: OriginFor<T>,
			collection: T::CollectionId,
			data: BoundedVec<u8, T::StringLimit>,
		) -> DispatchResult {
			pallet_nfts::Pallet::<T>::set_collection_metadata(origin, collection, data)
		}

		#[pallet::call_index(27)]
		#[pallet::weight(<T as pallet_nfts::Config>::WeightInfo::clear_collection_metadata())]
		pub fn clear_collection_metadata(
			origin: OriginFor<T>,
			collection: T::CollectionId,
		) -> DispatchResult {
			pallet_nfts::Pallet::<T>::clear_collection_metadata(origin, collection)
		}

		#[pallet::call_index(28)]
		#[pallet::weight(<T as pallet_nfts::Config>::WeightInfo::set_accept_ownership())]
		pub fn set_accept_ownership(
			origin: OriginFor<T>,
			maybe_collection: Option<T::CollectionId>,
		) -> DispatchResult {
			pallet_nfts::Pallet::<T>::set_accept_ownership(origin, maybe_collection)
		}

		#[pallet::call_index(29)]
		#[pallet::weight(<T as pallet_nfts::Config>::WeightInfo::set_collection_max_supply())]
		pub fn set_collection_max_supply(
			origin: OriginFor<T>,
			collection: T::CollectionId,
			max_supply: u32,
		) -> DispatchResult {
			pallet_nfts::Pallet::<T>::set_collection_max_supply(origin, collection, max_supply)
		}

		#[pallet::call_index(30)]
		#[pallet::weight(<T as pallet_nfts::Config>::WeightInfo::update_mint_settings())]
		pub fn update_mint_settings(
			origin: OriginFor<T>,
			collection: T::CollectionId,
			mint_settings: MintSettings<BalanceOf<T>, BlockNumberFor<T>, T::CollectionId>,
		) -> DispatchResult {
			pallet_nfts::Pallet::<T>::update_mint_settings(origin, collection, mint_settings)
		}

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

		#[pallet::call_index(33)]
		#[pallet::weight(<T as pallet_nfts::Config>::WeightInfo::pay_tips(tips.len() as u32))]
		pub fn pay_tips(
			origin: OriginFor<T>,
			tips: BoundedVec<ItemTipOf<T>, T::MaxTips>,
		) -> DispatchResult {
			pallet_nfts::Pallet::<T>::pay_tips(origin, tips)
		}

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

		#[pallet::call_index(35)]
		#[pallet::weight(<T as pallet_nfts::Config>::WeightInfo::cancel_swap())]
		pub fn cancel_swap(
			origin: OriginFor<T>,
			offered_collection: T::CollectionId,
			offered_item: T::ItemId,
		) -> DispatchResult {
			pallet_nfts::Pallet::<T>::cancel_swap(origin, offered_collection, offered_item)
		}

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
		fn increment_collection_id(collection: &T::CollectionId) -> T::NumericItemId {
			NextItemId::<T>::mutate(&collection, |next_item_id| {
				let val = next_item_id.clone();
				*next_item_id = next_item_id.saturating_add(One::one());
				val
			})
		}
	}
}
