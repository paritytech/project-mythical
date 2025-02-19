//! # ResetCollatorStakingRewardsRuntimeMigration
//!
//! This migration is responsible for resetting the collator staking rewards.
//! It ensures that any polluted state pertaining to `pallet-collator-staking` is cleared
//! from the chain, allowing the new logic in the pallet to take effect without issues.
//!
//! The migration clears all per-session rewards and claimable rewards, ensuring
//! a clean state moving forward. Note this migration must be applied only once.

use crate::Runtime;
use frame_support::{traits::OnRuntimeUpgrade, weights::Weight};
use sp_runtime::RuntimeDebug;

#[derive(RuntimeDebug)]
pub struct ResetCollatorStakingRewardsRuntimeMigration;

impl OnRuntimeUpgrade for ResetCollatorStakingRewardsRuntimeMigration {
	fn on_runtime_upgrade() -> Weight {
		log::info!("Starting ResetCollatorStakingRewardsRuntimeMigration...");

		let iterations =
			pallet_collator_staking::PerSessionRewards::<Runtime>::clear(u32::MAX, None).loops
				as u64;
		pallet_collator_staking::ClaimableRewards::<Runtime>::kill();

		log::info!("ResetCollatorStakingRewardsRuntimeMigration completed.");

		<Runtime as frame_system::Config>::DbWeight::get()
			.reads_writes(iterations, iterations.saturating_add(1))
	}
}
