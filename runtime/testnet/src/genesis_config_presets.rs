use super::Balance;
use super::{
	AccountId, AuraId, BalancesConfig, CollatorStakingConfig, CouncilConfig, ParachainInfoConfig,
	PolkadotXcmConfig, RuntimeGenesisConfig, SessionConfig, SessionKeys, SudoConfig, MUSE,
};
use alloc::vec;
use alloc::vec::Vec;
use cumulus_primitives_core::ParaId;
use frame_support::build_struct_json_patch;
use hex_literal::hex;
use runtime_common::{get_account_id_from_seed, get_collator_keys_from_seed, SAFE_XCM_VERSION};
use serde_json::{to_string, Value};
use sp_core::{crypto::UncheckedInto, ecdsa};
use sp_genesis_builder::{PresetId, DEV_RUNTIME_PRESET};
use sp_runtime::Percent;

pub const MUSE_RUNTIME_PRESET: &str = "muse";
pub const PARA_ID: u32 = 3369;

fn create_preset(
	invulnerables: Vec<(AccountId, AuraId)>,
	endowed_accounts: Vec<(AccountId, Balance)>,
	council: Vec<AccountId>,
	root_key: AccountId,
	id: ParaId,
) -> Value {
	build_struct_json_patch!(RuntimeGenesisConfig {
		balances: BalancesConfig { balances: endowed_accounts },
		parachain_info: ParachainInfoConfig { parachain_id: id },
		collator_staking: CollatorStakingConfig {
			invulnerables: invulnerables.iter().cloned().map(|(acc, _)| acc).collect::<Vec<_>>(),
			min_candidacy_bond: 50 * MUSE,
			min_stake: 10 * MUSE,
			desired_candidates: 6,
			collator_reward_percentage: Percent::from_parts(10),
			extra_reward: 0,
		},
		council: CouncilConfig { members: council },
		session: SessionConfig {
			keys: invulnerables
				.into_iter()
				.map(|(acc, aura)| {
					(
						acc,                  // account id
						acc,                  // validator id
						SessionKeys { aura }, // session keys
					)
				})
				.collect::<Vec<_>>(),
		},
		sudo: SudoConfig { key: Some(root_key) },
		polkadot_xcm: PolkadotXcmConfig { safe_xcm_version: Some(SAFE_XCM_VERSION) },
	})
}

pub fn get_builtin_preset(id: &PresetId) -> Option<Vec<u8>> {
	let preset = match id.as_ref() {
		DEV_RUNTIME_PRESET => {
			let balance_per_account = (1_000_000_000 * MUSE).saturating_div(6);
			create_preset(
				vec![
					(
						get_account_id_from_seed::<ecdsa::Public>("Alice"),
						get_collator_keys_from_seed("Alice"),
					),
					(
						get_account_id_from_seed::<ecdsa::Public>("Bob"),
						get_collator_keys_from_seed("Bob"),
					),
				],
				vec![
					(
						AccountId::from(hex!("f24FF3a9CF04c71Dbc94D0b566f7A27B94566cac")),
						balance_per_account,
					), // Alith
					(
						AccountId::from(hex!("3Cd0A705a2DC65e5b1E1205896BaA2be8A07c6e0")),
						balance_per_account,
					), // Baltathar
					(
						AccountId::from(hex!("798d4Ba9baf0064Ec19eB4F0a1a45785ae9D6DFc")),
						balance_per_account,
					), // Charleth
					(
						AccountId::from(hex!("773539d4Ac0e786233D90A233654ccEE26a613D9")),
						balance_per_account,
					), // Dorothy
					(
						AccountId::from(hex!("Ff64d3F6efE2317EE2807d223a0Bdc4c0c49dfDB")),
						balance_per_account,
					), // Ethan
					(
						AccountId::from(hex!("C0F0f4ab324C46e55D02D0033343B4Be8A55532d")),
						balance_per_account,
					), // Faith
				],
				vec![
					AccountId::from(hex!("3Cd0A705a2DC65e5b1E1205896BaA2be8A07c6e0")), // Baltathar
					AccountId::from(hex!("798d4Ba9baf0064Ec19eB4F0a1a45785ae9D6DFc")), // Charleth
					AccountId::from(hex!("773539d4Ac0e786233D90A233654ccEE26a613D9")), // Dorothy
				],
				AccountId::from(hex!("f24FF3a9CF04c71Dbc94D0b566f7A27B94566cac")),
				PARA_ID.into(),
			)
		},
		MUSE_RUNTIME_PRESET => create_preset(
			vec![
				(
					hex!("e6b4f55209a70384db3d147c06b99e32feb03d6fe191ff62b9dd23d5dd9ac64a").into(),
					hex!("e6b4f55209a70384db3d147c06b99e32feb03d6fe191ff62b9dd23d5dd9ac64a")
						.unchecked_into(),
				),
				(
					hex!("e07113e692708775d0cc39e00fe7f2974bff4e20a6fd127f0810c01142547723").into(),
					hex!("e07113e692708775d0cc39e00fe7f2974bff4e20a6fd127f0810c01142547723")
						.unchecked_into(),
				),
			],
			vec![
				(
					AccountId::from(hex!("16A5094837B65f1177824F0D36002f33d9A2Df7d")),
					150_000_000 * MUSE,
				),
				(AccountId::from(hex!("8CC95e7DFa96A86D728D2E6EB86400DEfBB56c90")), 1_000 * MUSE),
			],
			vec![],
			AccountId::from(hex!("8CC95e7DFa96A86D728D2E6EB86400DEfBB56c90")),
			PARA_ID.into(),
		),
		_ => return None,
	};

	Some(
		to_string(&preset)
			.expect("serialization to json is expected to work. qed.")
			.into_bytes(),
	)
}

pub fn preset_names() -> Vec<PresetId> {
	vec![PresetId::from(DEV_RUNTIME_PRESET), PresetId::from(MUSE_RUNTIME_PRESET)]
}
