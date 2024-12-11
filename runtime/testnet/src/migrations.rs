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
			hex!("e07113E692708775d0Cc39E00Fe7f2974bFF4e20"),
			hex!("E6b4f55209A70384dB3D147C06b99E32fEB03d6F"),
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
