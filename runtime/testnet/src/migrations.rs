use frame_support::traits::OnRuntimeUpgrade;
use frame_support::weights::Weight;
use hex_literal::hex;
use pallet_collator_staking::WeightInfo;
use sp_runtime::Percent;
use sp_std::vec;

use crate::{CollatorSelection, Runtime, RuntimeOrigin, MUSE};

pub struct CollatorSelectionSetupMigration;
impl OnRuntimeUpgrade for CollatorSelectionSetupMigration {
	fn on_runtime_upgrade() -> Weight {
		log::info!("Performing CollatorSelectionSetupMigration");
		let mut total_weight = Weight::zero();

		// Add invulnerables
		let invulnerables = vec![
			hex!("25451A4de12dcCc2D166922fA938E900fCc4ED24"),
			hex!("E04CC55ebEE1cBCE552f250e85c57B70B2E2625b"),
		];
		for invulnerable in invulnerables {
			if let Ok(result) =
				CollatorSelection::add_invulnerable(RuntimeOrigin::root(), invulnerable.into())
			{
				if let Some(weight) = result.actual_weight {
					total_weight.saturating_accrue(weight);
				}
			}
		}

		// Candidacy bond
		if let Ok(_) = CollatorSelection::set_candidacy_bond(RuntimeOrigin::root(), 100 * MUSE) {
			total_weight.saturating_accrue(
				<Runtime as pallet_collator_staking::Config>::WeightInfo::set_candidacy_bond(),
			);
		}

		// MinStake
		if let Ok(_) = CollatorSelection::set_minimum_stake(RuntimeOrigin::root(), 10 * MUSE) {
			total_weight.saturating_accrue(
				<Runtime as pallet_collator_staking::Config>::WeightInfo::set_minimum_stake(),
			);
		}

		// DesiredCandidates
		if let Ok(_) = CollatorSelection::set_desired_candidates(RuntimeOrigin::root(), 5) {
			total_weight.saturating_accrue(
				<Runtime as pallet_collator_staking::Config>::WeightInfo::set_desired_candidates(),
			);
		}

		// Collator reward percentage
		if let Ok(_) = CollatorSelection::set_collator_reward_percentage(
			RuntimeOrigin::root(),
			Percent::from_parts(20),
		) {
			total_weight.saturating_accrue(
				<Runtime as pallet_collator_staking::Config>::WeightInfo::set_collator_reward_percentage(),
			);
		}

		log::info!("CollatorSelectionSetupMigration executed");
		total_weight
	}
}
