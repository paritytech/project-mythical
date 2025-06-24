use super::Balance;
use super::{
	AccountId, AuraId, BalancesConfig, CollatorStakingConfig, CouncilConfig, ParachainInfoConfig,
	PolkadotXcmConfig, RuntimeGenesisConfig, SessionConfig, SessionKeys, SudoConfig, MYTH,
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

pub const MYTHOS_RUNTIME_PRESET: &str = "mythos";
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
			min_candidacy_bond: 50 * MYTH,
			min_stake: 10 * MYTH,
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
			let balance_per_account = (1_000_000_000 * MYTH).saturating_div(6);
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
		MYTHOS_RUNTIME_PRESET => create_preset(
			vec![
				(
					hex!("65c39EB8DDC9EA6F2135A28Ea670E97bc3CCc012").into(),
					hex!("d609c361de761b4bf8ba1ae4f8e436e74e1324b0a9eac08b34e31413bbd3f27f")
						.unchecked_into(),
				),
				(
					hex!("B9717024eB621a7AE331F92C3dC63a0aB60031c5").into(),
					hex!("8abe92437bf6690bc8f75cea612a5898cd2823c23681b346f776337660316979")
						.unchecked_into(),
				),
				(
					hex!("F4d1C38f3Be73d7cD2123968141Aec3AbB393153").into(),
					hex!("86360126eb30d60c9232206ba78a9fafb2322958bb3a021fa88ba09dfc753802")
						.unchecked_into(),
				),
				(
					hex!("E4f607AB7fA6b5Fd4f8127E051f151DaBb7279c6").into(),
					hex!("b0909f6832d2f5120b874b3e1cbe1b72fb5ccdbc268ba79bebdd8e71ab41e334")
						.unchecked_into(),
				),
			],
			vec![
				(AccountId::from(hex!("742c722892976C23A3919ADC7A4B562169B91E41")), 1_000 * MYTH),
				(
					AccountId::from(hex!("f476dA221b07135b106d923b8884b76b09982B4F")),
					150_000_000 * MYTH,
				),
			],
			vec![],
			AccountId::from(hex!("742c722892976C23A3919ADC7A4B562169B91E41")),
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
	vec![PresetId::from(DEV_RUNTIME_PRESET), PresetId::from(MYTHOS_RUNTIME_PRESET)]
}
