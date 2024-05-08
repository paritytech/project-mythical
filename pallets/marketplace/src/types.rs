use frame_support::traits::fungible::Inspect;
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_std::vec::Vec;

use crate::Config;

pub type BalanceOf<T> =
	<<T as Config>::Currency as Inspect<<T as frame_system::Config>::AccountId>>::Balance;

pub type OrderOf<T> = Order<
	<T as pallet_nfts::Config>::CollectionId,
	<T as pallet_nfts::Config>::ItemId,
	BalanceOf<T>,
	<T as pallet_timestamp::Config>::Moment,
	<T as Config>::Signature,
	Vec<u8>,
	<T as frame_system::Config>::AccountId,
>;

pub type OrderMessageOf<T> = OrderMessage<
	<T as pallet_nfts::Config>::CollectionId,
	<T as pallet_nfts::Config>::ItemId,
	BalanceOf<T>,
	<T as pallet_timestamp::Config>::Moment,
	Vec<u8>,
	<T as frame_system::Config>::AccountId,
>;

#[derive(Clone, Encode, Decode, Debug, Eq, PartialEq, TypeInfo, MaxEncodedLen)]
pub struct Ask<AccountId, Amount, Expiration> {
	pub seller: AccountId,
	pub price: Amount,
	pub expiration: Expiration,
	pub fee: Amount,
	pub escrow_agent: Option<AccountId>,
}

#[derive(Clone, Encode, Decode, Debug, Eq, PartialEq, TypeInfo, MaxEncodedLen)]
pub struct Bid<AccountId, Amount, Expiration> {
	pub buyer: AccountId,
	pub expiration: Expiration,
	pub fee: Amount,
}

#[derive(Clone, Encode, Decode, Debug, Eq, PartialEq, TypeInfo)]
pub enum OrderType {
	Ask,
	Bid,
}

#[derive(Clone, Encode, Decode, Debug, Eq, PartialEq, TypeInfo)]
pub enum ExecOrder<AccountId, Amount, Expiration> {
	Ask(Ask<AccountId, Amount, Expiration>),
	Bid(Bid<AccountId, Amount, Expiration>),
}

#[derive(Clone, Encode, Decode, Debug, Eq, PartialEq, TypeInfo)]
pub struct Order<
	CollectionId,
	ItemId,
	Amount,
	Expiration,
	OffchainSignature,
	BoundedString,
	AccountId,
> {
	pub order_type: OrderType,
	pub collection: CollectionId,
	pub item: ItemId,
	pub price: Amount,
	pub expires_at: Expiration,
	pub fee: Amount,
	pub escrow_agent: Option<AccountId>,
	pub signature_data: SignatureData<OffchainSignature, BoundedString>,
}

#[derive(Clone, Encode, Decode, Debug, Eq, PartialEq, TypeInfo)]
pub struct SignatureData<OffchainSignature, BoundedString> {
	pub signature: OffchainSignature,
	pub nonce: BoundedString,
}

///Message data to be signed by the fee_signer account
#[derive(Clone, Encode, Decode, Debug, Eq, PartialEq, TypeInfo)]
pub struct OrderMessage<CollectionId, ItemId, Amount, Expiration, BoundedString, AccountId> {
	pub collection: CollectionId,
	pub item: ItemId,
	pub price: Amount,
	pub expires_at: Expiration,
	pub fee: Amount,
	pub escrow_agent: Option<AccountId>,
	pub nonce: BoundedString,
}

impl<CollectionId, ItemId, Amount, Expiration, OffchainSignature, BoundedString, AccountId>
	From<
		Order<
			CollectionId,
			ItemId,
			Amount,
			Expiration,
			OffchainSignature,
			BoundedString,
			AccountId,
		>,
	> for OrderMessage<CollectionId, ItemId, Amount, Expiration, BoundedString, AccountId>
{
	fn from(
		x: Order<
			CollectionId,
			ItemId,
			Amount,
			Expiration,
			OffchainSignature,
			BoundedString,
			AccountId,
		>,
	) -> Self {
		OrderMessage {
			collection: x.collection,
			item: x.item,
			price: x.price,
			expires_at: x.expires_at,
			fee: x.fee,
			escrow_agent: x.escrow_agent,
			nonce: x.signature_data.nonce,
		}
	}
}

#[derive(Clone, Encode, Decode, Debug, Eq, PartialEq, TypeInfo)]
pub enum Execution {
	/// The order must be executed otherwise it should fail
	Force,
	/// If order execution is not possible create the order on storage
	AllowCreation,
}

#[cfg(feature = "runtime-benchmarks")]
pub trait BenchmarkHelper<CollectionId, ItemId, Moment> {
	/// Returns a collection id from a given integer.
	fn collection(id: u16) -> CollectionId;
	/// Returns an nft id from a given integer.
	fn item(id: u16) -> ItemId;
	fn timestamp(value: u64) -> Moment;
}
