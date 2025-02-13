#![cfg_attr(not(feature = "std"), no_std)]

pub mod burner_adapter;

use core::marker::PhantomData;
use frame_support::traits::{Get, OriginTrait};
use xcm::latest::prelude::*;
use xcm_executor::traits::{FeeReason, TransactAsset};

// Convert a local Origin (i.e., a signed 20 byte account Origin) to a Location
pub struct SignedToAccountId20<Origin, AccountId, Network>(
	sp_std::marker::PhantomData<(Origin, AccountId, Network)>,
);
impl<Origin: OriginTrait + Clone, AccountId: Into<[u8; 20]>, Network: Get<NetworkId>>
	sp_runtime::traits::TryConvert<Origin, Location>
	for SignedToAccountId20<Origin, AccountId, Network>
where
	Origin::PalletsOrigin: From<frame_system::RawOrigin<AccountId>>
		+ TryInto<frame_system::RawOrigin<AccountId>, Error = Origin::PalletsOrigin>,
{
	fn try_convert(o: Origin) -> Result<Location, Origin> {
		o.try_with_caller(|caller| match caller.try_into() {
			Ok(frame_system::RawOrigin::Signed(who)) => {
				Ok(AccountKey20 { key: who.into(), network: Some(Network::get()) }.into())
			},
			Ok(other) => Err(other.into()),
			Err(other) => Err(other),
		})
	}
}

/// Try to deposit the given fee in the specified account.
/// Burns the fee in case of a failure.
pub fn deposit_or_burn_fee<AssetTransactor: TransactAsset, AccountId: Clone + Into<[u8; 20]>>(
	fee: Assets,
	context: Option<&XcmContext>,
	receiver: AccountId,
) {
	let dest = AccountKey20 { network: None, key: receiver.into() }.into();
	for asset in fee.into_inner() {
		if let Err(e) = AssetTransactor::deposit_asset(&asset, &dest, context) {
			log::trace!(
				target: "xcm::fees",
				"`AssetTransactor::deposit_asset` returned error: {:?}. Burning fee: {:?}. \
				They might be burned.",
				e, asset,
			);
		}
	}
}

/// A `HandleFee` implementation that simply deposits the fees into a specific on-chain
/// `ReceiverAccount`.
///
/// It reuses the `AssetTransactor` configured on the XCM executor to deposit fee assets. If
/// the `AssetTransactor` returns an error while calling `deposit_asset`, then a warning will be
/// logged and the fee burned.
pub struct XcmFeeToAccountId20<AssetTransactor, AccountId, ReceiverAccount>(
	PhantomData<(AssetTransactor, AccountId, ReceiverAccount)>,
);

impl<
		AssetTransactor: TransactAsset,
		AccountId: Clone + Into<[u8; 20]>,
		ReceiverAccount: Get<AccountId>,
	> xcm_builder::HandleFee for XcmFeeToAccountId20<AssetTransactor, AccountId, ReceiverAccount>
{
	fn handle_fee(fee: Assets, context: Option<&XcmContext>, _reason: FeeReason) -> Assets {
		deposit_or_burn_fee::<AssetTransactor, _>(fee, context, ReceiverAccount::get());

		Assets::new()
	}
}
