use frame_support::traits::OnRuntimeUpgrade;
use frame_support::weights::Weight;
use hex_literal::hex;
use pallet_collator_staking::WeightInfo;
use sp_runtime::Percent;
use sp_std::vec;

use crate::{CollatorStaking, Runtime, RuntimeOrigin, MYTH};

pub struct CollatorStakingSetupMigration;
impl OnRuntimeUpgrade for CollatorStakingSetupMigration {
	fn on_runtime_upgrade() -> Weight {
		log::info!("Performing CollatorStakingSetupMigration");
		let mut total_weight = Weight::zero();

		// Add invulnerables
		let invulnerables = vec![
			hex!("65c39EB8DDC9EA6F2135A28Ea670E97bc3CCc012"),
			hex!("B9717024eB621a7AE331F92C3dC63a0aB60031c5"),
			hex!("E4f607AB7fA6b5Fd4f8127E051f151DaBb7279c6"),
			hex!("F4d1C38f3Be73d7cD2123968141Aec3AbB393153"),
		];
		for (i, invulnerable) in invulnerables.into_iter().enumerate() {
			let result =
				CollatorStaking::add_invulnerable(RuntimeOrigin::root(), invulnerable.into());
			match result {
				Ok(info) => {
					if let Some(weight) = info.actual_weight {
						total_weight.saturating_accrue(weight);
					} else {
						total_weight.saturating_accrue(
							<Runtime as pallet_collator_staking::Config>::WeightInfo::add_invulnerable(
								i as u32,
							),
						)
					}
				},
				Err(e) => log::warn!("An error occurred adding an invulnerable: {:?}", e),
			}
		}

		// Candidacy bond
		if CollatorStaking::set_min_candidacy_bond(RuntimeOrigin::root(), 5_000 * MYTH).is_ok() {
			total_weight.saturating_accrue(
				<Runtime as pallet_collator_staking::Config>::WeightInfo::set_min_candidacy_bond(),
			);
		}

		// MinStake
		if CollatorStaking::set_minimum_stake(RuntimeOrigin::root(), 500 * MYTH).is_ok() {
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
