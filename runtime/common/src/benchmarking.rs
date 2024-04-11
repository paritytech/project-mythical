use crate::AccountId;
use pallet_treasury::ArgumentsFactory;

pub struct TreasuryBenchmarkHelper;
impl ArgumentsFactory<(), AccountId> for TreasuryBenchmarkHelper {
	fn create_asset_kind(_seed: u32) -> () {
		()
	}

	fn create_beneficiary(seed: [u8; 32]) -> AccountId {
		AccountId::from(seed)
	}
}
