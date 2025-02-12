use crate::Balance;
use core::marker::PhantomData;
use xcm::latest::prelude::{Asset, Location, XcmContext, XcmError, XcmResult};
use xcm_executor::{
	traits::{MatchesFungible, TransactAsset},
	AssetsInHolding,
};
pub struct BurnerAdapter<Matcher>(PhantomData<Matcher>);
impl<Matcher: MatchesFungible<Balance>> TransactAsset for BurnerAdapter<Matcher> {
	fn can_check_in(_origin: &Location, what: &Asset, _context: &XcmContext) -> XcmResult {
		log::trace!(
			target: "xcm::burner_adapter",
			"can_check_in origin: {:?}, what: {:?}",
			_origin, what
		);
		match Matcher::matches_fungible(what) {
			Some(_) => Ok(()),
			None => Err(XcmError::AssetNotFound),
		}
	}

	fn check_in(_origin: &Location, _what: &Asset, _context: &XcmContext) {
		// No-op
	}

	fn can_check_out(_dest: &Location, what: &Asset, _context: &XcmContext) -> XcmResult {
		log::trace!(
			target: "xcm::burner_adapter",
			"can_check_out dest: {:?}, what: {:?}",
			_dest, what
		);
		match Matcher::matches_fungible(what) {
			Some(_) => Err(XcmError::Unimplemented),
			None => Err(XcmError::AssetNotFound),
		}
	}

	fn check_out(_dest: &Location, _what: &Asset, _context: &XcmContext) {
		// No-op
	}

	fn deposit_asset(what: &Asset, _who: &Location, _context: Option<&XcmContext>) -> XcmResult {
		// Only accept and do nothing with the matched asset
		log::trace!(
			target: "xcm::burner_adapter",
			"deposit_asset what: {:?}, who: {:?}",
			what, _who,
		);
		match Matcher::matches_fungible(what) {
			Some(_) => Ok(()),
			None => Err(XcmError::AssetNotFound),
		}
	}

	fn withdraw_asset(
		what: &Asset,
		_who: &Location,
		_maybe_context: Option<&XcmContext>,
	) -> Result<AssetsInHolding, XcmError> {
		log::trace!(
			target: "xcm::burner_adapter",
			"withdraw_asset called with asset: {:?}, who: {:?}, context: {:?}",
			what, _who, _maybe_context
		);
		let matches = Matcher::matches_fungible(what);
		match matches {
			Some(_) => {
				log::trace!(
					target: "xcm::burner_adapter",
					// Error propagrates as `AssetNotFound` in executor therefore the log.
					"returning Unimplemented as we don't support withdrawals"
				);
				Err(XcmError::Unimplemented)
			},
			None => Err(XcmError::AssetNotFound),
		}
	}

	fn internal_transfer_asset(
		asset: &Asset,
		_from: &Location,
		_to: &Location,
		_context: &XcmContext,
	) -> Result<AssetsInHolding, XcmError> {
		log::trace!(
			target: "xcm::burner_adapter",
			"internal_transfer_asset asset: {:?}, from: {:?}, to: {:?}",
			asset, _from, _to,
		);
		match Matcher::matches_fungible(asset) {
			Some(_) => Err(XcmError::Unimplemented),
			None => Err(XcmError::AssetNotFound),
		}
	}
}
