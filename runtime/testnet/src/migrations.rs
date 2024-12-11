use frame_support::traits::OnRuntimeUpgrade;
use frame_support::weights::Weight;
use hex_literal::hex;
use pallet_collator_staking::WeightInfo;
use sp_runtime::Percent;
use sp_std::vec;

use crate::{CollatorStaking, Runtime, RuntimeOrigin, MUSE};

pub struct CollatorStakingSetupMigration;
impl OnRuntimeUpgrade for CollatorStakingSetupMigration {
	fn on_runtime_upgrade() -> Weight {
		log::info!("Performing CollatorStakingSetupMigration");
		let mut total_weight = Weight::zero();

		// Add invulnerables
		let invulnerables = vec![
			hex!("25451A4de12dcCc2D166922fA938E900fCc4ED24"),
			hex!("E04CC55ebEE1cBCE552f250e85c57B70B2E2625b"),
		];
		for (i, invulnerable) in invulnerables.into_iter().enumerate() {
			if let Ok(result) =
				CollatorStaking::add_invulnerable(RuntimeOrigin::root(), invulnerable.into())
			{
				if let Some(weight) = result.actual_weight {
					total_weight.saturating_accrue(weight);
				} else {
					total_weight.saturating_accrue(
						<Runtime as pallet_collator_staking::Config>::WeightInfo::add_invulnerable(
							i as u32,
						),
					)
				}
			}
		}

		// Candidacy bond
		if CollatorStaking::set_min_candidacy_bond(RuntimeOrigin::root(), 50 * MUSE).is_ok() {
			total_weight.saturating_accrue(
				<Runtime as pallet_collator_staking::Config>::WeightInfo::set_min_candidacy_bond(),
			);
		}

		// MinStake
		if CollatorStaking::set_minimum_stake(RuntimeOrigin::root(), 10 * MUSE).is_ok() {
			total_weight.saturating_accrue(
				<Runtime as pallet_collator_staking::Config>::WeightInfo::set_minimum_stake(),
			);
		}

		// DesiredCandidates
		if CollatorStaking::set_desired_candidates(RuntimeOrigin::root(), 6).is_ok() {
			total_weight.saturating_accrue(
				<Runtime as pallet_collator_staking::Config>::WeightInfo::set_desired_candidates(),
			);
		}

		// Collator reward percentage
		if CollatorStaking::set_collator_reward_percentage(
			RuntimeOrigin::root(),
			Percent::from_parts(10),
		)
		.is_ok()
		{
			total_weight.saturating_accrue(
				<Runtime as pallet_collator_staking::Config>::WeightInfo::set_collator_reward_percentage(),
			);
		}

		log::info!("CollatorStakingSetupMigration successfully executed");
		total_weight
	}
}
