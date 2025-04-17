use crate::Config;
use frame_support::traits::fungible::Inspect;
use parity_scale_codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use scale_info::TypeInfo;

pub type BalanceOf<T> =
	<<T as Config>::Currency as Inspect<<T as frame_system::Config>::AccountId>>::Balance;

pub type Item = u128;
pub type Domain = [u8; 8];

#[derive(Clone, Encode, Decode, DecodeWithMemTracking, Debug, Eq, PartialEq, TypeInfo)]
pub struct TradeParams<Amount, ItemId, Expiration> {
	pub price: Amount,
	pub fee: Amount,
	pub item: ItemId,
	pub ask_expiration: Expiration,
	pub bid_expiration: Expiration,
}

pub type TradeParamsOf<T> = TradeParams<
	<<T as Config>::Currency as Inspect<<T as frame_system::Config>::AccountId>>::Balance,
	Item,
	<T as pallet_timestamp::Config>::Moment,
>;

#[derive(Clone, Encode, Decode, DecodeWithMemTracking, Debug, Eq, PartialEq, TypeInfo)]
pub struct TradeSignatures<OffchainSignature> {
	pub ask_signature: OffchainSignature,
	pub bid_signature: OffchainSignature,
}

pub type TradeSignaturesOf<T> = TradeSignatures<<T as pallet_nfts::Config>::OffchainSignature>;

#[derive(Clone, Encode, Decode, Debug, Eq, PartialEq, TypeInfo)]
pub struct AskMessage<Account, Amount, ItemId, Expiration> {
	pub domain: Domain,
	pub sender: Account,
	pub fee_address: Account,
	pub item: ItemId,
	pub price: Amount,
	pub expiration: Expiration,
}

pub type AskMessageOf<T> = AskMessage<
	<T as frame_system::Config>::AccountId,
	<<T as Config>::Currency as Inspect<<T as frame_system::Config>::AccountId>>::Balance,
	Item,
	<T as pallet_timestamp::Config>::Moment,
>;

#[derive(Clone, Encode, Decode, Debug, Eq, PartialEq, TypeInfo, MaxEncodedLen)]
pub struct OrderData<Account> {
	pub caller: Account,
	pub fee_address: Account,
}

pub type OrderDataOf<T> = OrderData<<T as frame_system::Config>::AccountId>;

#[derive(Clone, Encode, Decode, Debug, Eq, PartialEq, TypeInfo)]
pub struct BidMessage<Account, Amount, ItemId, Expiration> {
	pub domain: Domain,
	pub sender: Account,
	pub fee_address: Account,
	pub item: ItemId,
	pub price: Amount,
	pub fee: Amount,
	pub expiration: Expiration,
}

pub type BidMessageOf<T> = BidMessage<
	<T as frame_system::Config>::AccountId,
	<<T as Config>::Currency as Inspect<<T as frame_system::Config>::AccountId>>::Balance,
	Item,
	<T as pallet_timestamp::Config>::Moment,
>;

#[cfg(feature = "runtime-benchmarks")]
pub trait BenchmarkHelper<CollectionId, Moment> {
	/// Returns a collection id from a given integer.
	fn collection(id: u16) -> CollectionId;
	/// Returns an nft id from a given integer.
	fn timestamp(value: u64) -> Moment;
}
