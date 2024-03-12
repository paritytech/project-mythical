#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::traits::{Get, OriginTrait};
use xcm::latest::prelude::*;

// Convert a local Origin (i.e., a signed 20 byte account Origin) to a Location
pub struct SignedToAccountId20<Origin, AccountId, Network>(
	sp_std::marker::PhantomData<(Origin, AccountId, Network)>,
);
impl<Origin: OriginTrait + Clone, AccountId: Into<[u8; 20]>, Network: Get<NetworkId>>
	sp_runtime::traits::TryConvert<Origin, Location> for SignedToAccountId20<Origin, AccountId, Network>
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
