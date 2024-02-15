use frame_support::traits::Currency;
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;

use crate::Config;

pub type BalanceOf<T> =
	<<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

pub type HashId = [u8; 32];

#[derive(Clone, Encode, Decode, Debug, Eq, PartialEq, TypeInfo, MaxEncodedLen)]
pub struct Ask<AccountId, Amount, Expiration> {
	pub seller: AccountId,
	pub price: Amount,
	pub expiration: Expiration,
	pub fee: Amount,
}

#[derive(Clone, Encode, Decode, Debug, Eq, PartialEq, TypeInfo, MaxEncodedLen)]
pub struct Bid<AccountId, Expiration, Amount> {
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
pub struct Order<CollectionId, ItemId, Amount, Expiration, BoundedString> {
	pub order_type: OrderType,
	pub collection: CollectionId,
	pub item: ItemId,
	pub price: Amount,
	pub expires_at: Expiration,
	pub fee_percent: Amount,
	pub signature_data: SignatureData<BoundedString>,
}

#[derive(Clone, Encode, Decode, Debug, Eq, PartialEq, TypeInfo)]
pub struct SignatureData<BoundedString> {
	pub signature: [u8; 32], //keccak256 signature
	pub nonce: BoundedString,
}

#[derive(Clone, Encode, Decode, Debug, Eq, PartialEq, TypeInfo, MaxEncodedLen)]
pub struct Suggestion<CollectionId, ItemId, Amount, AccountId> {
	pub collection: CollectionId,
	pub item: ItemId,
	pub price: Amount,
	pub fee_percent: Amount,
	pub suggestion_fill: SuggestionFill<AccountId, Amount>,
}

#[derive(Clone, Encode, Decode, Debug, Eq, PartialEq, TypeInfo, MaxEncodedLen)]
pub struct SuggestionFill<AccountId, Amount> {
	who: AccountId,
	value: Amount,
	fee: Amount,
}

#[derive(Clone, Encode, Decode, Debug, Eq, PartialEq, TypeInfo, MaxEncodedLen)]
pub struct WantAsk<CollectionId, ItemId, Amount> {
	pub collection: CollectionId,
	pub item: ItemId,
	pub price: Amount,
	pub fee_percent: Amount,
}
#[derive(Clone, Encode, Decode, Debug, PartialEq, Eq, TypeInfo)]
pub struct ExecSuggestion<BoundedString> {
	pub item_key: HashId,
	pub ask_signature: SignatureData<BoundedString>,
	pub bid_signature: SignatureData<BoundedString>,
}

#[derive(Clone, Encode, Decode, Debug, PartialEq, Eq, TypeInfo)]
pub struct ExecWantAsk<BoundedString> {
	pub item_key: HashId,
	pub bid_signature: SignatureData<BoundedString>,
}

#[derive(Clone, Encode, Decode, Debug, Eq, PartialEq, TypeInfo, MaxEncodedLen)]
pub struct Exchange<AccountId, Expiration, Amount> {
	pub creator: AccountId,
	pub expiration: Expiration,
	pub executed: bool,
	pub extra_value: Amount,
}
