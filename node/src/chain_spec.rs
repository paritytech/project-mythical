use cumulus_primitives_core::ParaId;
use hex_literal::hex;
use runtime_common::{AccountId, AuraId, EthereumSignature};
use sc_chain_spec::{ChainSpecExtension, ChainSpecGroup};
use sc_service::ChainType;
use serde::{Deserialize, Serialize};
use sp_core::{crypto::UncheckedInto, ecdsa, Pair, Public};
use sp_runtime::traits::{IdentifyAccount, Verify};

use mythical_devnet;
use mythical_mainnet;

/// Specialized `ChainSpec` for the normal parachain runtime.
pub type MainChainSpec =
	sc_service::GenericChainSpec<mythical_mainnet::RuntimeGenesisConfig, Extensions>;

/// Specialized `ChainSpec` for the development parachain runtime.
pub type DevnetChainSpec =
	sc_service::GenericChainSpec<mythical_devnet::RuntimeGenesisConfig, Extensions>;

/// The default XCM version to set in genesis config.
const SAFE_XCM_VERSION: u32 = xcm::prelude::XCM_VERSION;

const PARA_ID: u32 = 201804;

/// Helper function to generate a crypto pair from seed
pub fn get_from_seed<TPublic: Public>(seed: &str) -> <TPublic::Pair as Pair>::Public {
	TPublic::Pair::from_string(&format!("//{}", seed), None)
		.expect("static values are valid; qed")
		.public()
}

/// The extensions for the [`ChainSpec`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ChainSpecGroup, ChainSpecExtension)]
#[serde(deny_unknown_fields)]
pub struct Extensions {
	/// The relay chain of the Parachain.
	pub relay_chain: String,
	/// The id of the Parachain.
	pub para_id: u32,
}

impl Extensions {
	/// Try to get the extension from the given `ChainSpec`.
	pub fn try_get(chain_spec: &dyn sc_service::ChainSpec) -> Option<&Self> {
		sc_chain_spec::get_extension(chain_spec.extensions())
	}
}

type AccountPublic = <EthereumSignature as Verify>::Signer;

/// Generate collator keys from seed.
///
/// This function's return type must always match the session keys of the chain in tuple format.
pub fn get_collator_keys_from_seed(seed: &str) -> AuraId {
	get_from_seed::<AuraId>(seed)
}

/// Helper function to generate an account ID from seed
pub fn get_account_id_from_seed<TPublic: Public>(seed: &str) -> AccountId
where
	AccountPublic: From<<TPublic::Pair as Pair>::Public>,
{
	AccountPublic::from(get_from_seed::<TPublic>(seed)).into_account()
}

/// Generate the session keys from individual elements.
///
/// The input must be a tuple of individual keys (a single arg for now since we have just one key).
pub fn mainnet_session_keys(keys: AuraId) -> mythical_mainnet::SessionKeys {
	mythical_mainnet::SessionKeys { aura: keys }
}

pub fn devnet_session_keys(keys: AuraId) -> mythical_devnet::SessionKeys {
	mythical_devnet::SessionKeys { aura: keys }
}

fn check_sudo_key(authority_set: &mut Vec<AccountId>, threshold: u16) {
	assert!(threshold > 0, "Threshold for sudo multisig cannot be 0");
	assert!(!authority_set.is_empty(), "Sudo authority set cannot be empty");
	assert!(
		authority_set.len() >= threshold.into(),
		"Threshold must be less than or equal to authority set members"
	);
	// Sorting is done to deterministically order the multisig set
	// So that a single authority set (A, B, C) may generate only a single unique multisig key
	// Otherwise, (B, A, C) or (C, A, B) could produce different keys and cause chaos
	authority_set.sort();
}

/// Generate a multisig key from a given `authority_set` and a `threshold`
/// Used for generating a multisig to use as sudo key for devnet.
pub fn get_devnet_multisig_sudo_key(
	mut authority_set: Vec<AccountId>,
	threshold: u16,
) -> AccountId {
	check_sudo_key(&mut authority_set, threshold);
	pallet_multisig::Pallet::<mythical_devnet::Runtime>::multi_account_id(
		&authority_set[..],
		threshold,
	)
}

/// Generate a multisig key from a given `authority_set` and a `threshold`
/// Used for generating a multisig to use as sudo key for mainnet.
pub fn get_mainnet_multisig_sudo_key(
	mut authority_set: Vec<AccountId>,
	threshold: u16,
) -> AccountId {
	check_sudo_key(&mut authority_set, threshold);
	pallet_multisig::Pallet::<mythical_mainnet::Runtime>::multi_account_id(
		&authority_set[..],
		threshold,
	)
}

pub mod devnet {
	use mythical_devnet::MUSE;

	use super::*;
	pub fn development_config() -> DevnetChainSpec {
		// Give your base currency a unit name and decimal places
		let mut properties = sc_chain_spec::Properties::new();
		properties.insert("tokenSymbol".into(), "MUSE".into());
		properties.insert("tokenDecimals".into(), 18.into());
		properties.insert("ss58Format".into(), 333.into());
		properties.insert("isEthereum".into(), true.into());

		DevnetChainSpec::builder(
			mythical_devnet::WASM_BINARY.expect("WASM binary was not built, please build it!"),
			Extensions {
				relay_chain: "rococo-local".into(), // You MUST set this to the correct network!
				para_id: PARA_ID,
			},
		)
		// Name
		.with_name("Development Muse Testnet")
		// ID
		.with_id("devnet_muse_network_dev")
		.with_chain_type(ChainType::Development)
		.with_genesis_config_patch(devnet_genesis(
			// initial collators.
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
				AccountId::from(hex!("f24FF3a9CF04c71Dbc94D0b566f7A27B94566cac")), // Alith
				AccountId::from(hex!("3Cd0A705a2DC65e5b1E1205896BaA2be8A07c6e0")), // Baltathar
				AccountId::from(hex!("798d4Ba9baf0064Ec19eB4F0a1a45785ae9D6DFc")), // Charleth
				AccountId::from(hex!("773539d4Ac0e786233D90A233654ccEE26a613D9")), // Dorothy
				AccountId::from(hex!("Ff64d3F6efE2317EE2807d223a0Bdc4c0c49dfDB")), // Ethan
				AccountId::from(hex!("C0F0f4ab324C46e55D02D0033343B4Be8A55532d")), // Faith
			],
			// Example multisig sudo key configuration:
			// Configures 2/3 threshold multisig key
			// Note: For using this multisig key as a sudo key, each individual signatory must possess funds
			get_devnet_multisig_sudo_key(
				vec![
					AccountId::from(hex!("f24FF3a9CF04c71Dbc94D0b566f7A27B94566cac")), // Alith
					AccountId::from(hex!("3Cd0A705a2DC65e5b1E1205896BaA2be8A07c6e0")), // Baltathar
					AccountId::from(hex!("798d4Ba9baf0064Ec19eB4F0a1a45785ae9D6DFc")), // Charleth
				],
				2,
			),
			PARA_ID.into(),
			Some(1_000_000_000 * MUSE),
		))
		.with_properties(properties)
		.build()
	}

	pub fn devnet_config() -> DevnetChainSpec {
		// Give your base currency a unit name and decimal places
		let mut properties = sc_chain_spec::Properties::new();
		properties.insert("tokenSymbol".into(), "MUSE".into());
		properties.insert("tokenDecimals".into(), 18.into());
		properties.insert("ss58Format".into(), 333.into());
		properties.insert("isEthereum".into(), true.into());

		DevnetChainSpec::builder(
			mythical_devnet::WASM_BINARY.expect("WASM binary was not built, please build it!"),
			Extensions {
				relay_chain: "rococo".into(), // You MUST set this to the correct network!
				para_id: PARA_ID,
			},
		)
		.with_name("Muse Testnet")
		.with_id("muse")
		.with_chain_type(ChainType::Live)
		.with_genesis_config_patch(devnet_genesis(
			// initial collators.
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
				AccountId::from(hex!("ad49e6384184719D6ECC24DFEB61BF4D181138D8")),
				AccountId::from(hex!("90D157d5d32A01f7d518A804f821315f07DE2042")),
				AccountId::from(hex!("4FbF551aF1269DEba03C85Dbe990bA10EA28BCc6")),
			],
			AccountId::from(hex!("4FbF551aF1269DEba03C85Dbe990bA10EA28BCc6")),
			PARA_ID.into(),
			Some(1_000_000_000 * MUSE),
		))
		.with_protocol_id("muse")
		.with_properties(properties)
		.build()
	}

	fn devnet_genesis(
		invulnerables: Vec<(AccountId, AuraId)>,
		endowed_accounts: Vec<AccountId>,
		root_key: AccountId,
		id: ParaId,
		total_issuance: Option<mythical_devnet::Balance>,
	) -> serde_json::Value {
		use mythical_devnet::EXISTENTIAL_DEPOSIT;
		//TODO: Define multisig root account
		//let alice = get_from_seed::<sr25519::Public>("Alice");
		//let bob = get_from_seed::<sr25519::Public>("Bob");

		let num_endowed_accounts = endowed_accounts.len();
		let balances = match total_issuance {
			Some(total_issuance) => {
				let balance_per_endowed = total_issuance
					.checked_div(num_endowed_accounts as mythical_devnet::Balance)
					.unwrap_or(0 as mythical_devnet::Balance);

				endowed_accounts.iter().cloned().map(|k| (k, balance_per_endowed)).collect()
			},
			None => vec![],
		};

		serde_json::json!({
				"balances": {
					"balances": balances
				},
				"parachainInfo": {
					"parachainId": id,
				},
				"collatorSelection": {
					"invulnerables": invulnerables.iter().cloned().map(|(acc, _)| acc).collect::<Vec<_>>(),
					"candidacyBond": EXISTENTIAL_DEPOSIT * 16,
				},
				"session": {
					"keys": invulnerables
						.into_iter()
						.map(|(acc, aura)| {
							(
								acc.clone(),               // account id
								acc,                       // validator id
								devnet_session_keys(aura), // session keys
							)
						})
						.collect::<Vec<_>>(),
				},
				"sudo": { "key": Some(root_key) },
				"polkadotXcm": {
					"safeXcmVersion": Some(SAFE_XCM_VERSION),
				},
			}
		)
	}
}

pub mod mainnet {
	use mythical_mainnet::MYTH;

	use super::*;
	pub fn development_config() -> MainChainSpec {
		// Give your base currency a unit name and decimal places
		let mut properties = sc_chain_spec::Properties::new();
		properties.insert("tokenSymbol".into(), "MYTH".into());
		properties.insert("tokenDecimals".into(), 18.into());
		properties.insert("ss58Format".into(), 333.into());
		properties.insert("isEthereum".into(), true.into());

		MainChainSpec::builder(
			mythical_mainnet::WASM_BINARY.expect("WASM binary was not build, please build it!"),
			Extensions {
				relay_chain: "polkadot-local".into(), // You MUST set this to the correct network!
				para_id: PARA_ID,
			},
		)
		// Name
		.with_name("Development MYTH Mainnet")
		// ID
		.with_id("mainnet_mythical_network_dev")
		.with_chain_type(ChainType::Development)
		.with_genesis_config_patch(mainnet_genesis(
			// initial collators.
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
				AccountId::from(hex!("f24FF3a9CF04c71Dbc94D0b566f7A27B94566cac")), // Alith
				AccountId::from(hex!("3Cd0A705a2DC65e5b1E1205896BaA2be8A07c6e0")), // Baltathar
				AccountId::from(hex!("798d4Ba9baf0064Ec19eB4F0a1a45785ae9D6DFc")), // Charleth
				AccountId::from(hex!("773539d4Ac0e786233D90A233654ccEE26a613D9")), // Dorothy
				AccountId::from(hex!("Ff64d3F6efE2317EE2807d223a0Bdc4c0c49dfDB")), // Ethan
				AccountId::from(hex!("C0F0f4ab324C46e55D02D0033343B4Be8A55532d")), // Faith
			],
			// Example multisig sudo key configuration:
			// Configures 2/3 threshold multisig key
			// Note: For using this multisig key as a sudo key, each individual signatory must possess funds
			get_mainnet_multisig_sudo_key(
				vec![
					AccountId::from(hex!("f24FF3a9CF04c71Dbc94D0b566f7A27B94566cac")), // Alith
					AccountId::from(hex!("3Cd0A705a2DC65e5b1E1205896BaA2be8A07c6e0")), // Baltathar
					AccountId::from(hex!("798d4Ba9baf0064Ec19eB4F0a1a45785ae9D6DFc")), // Charleth
				],
				2,
			),
			PARA_ID.into(),
			Some(1_000_000_000 * MYTH),
		))
		.with_properties(properties)
		.build()
	}

	pub fn mainnet_config() -> MainChainSpec {
		// Give your base currency a unit name and decimal places
		let mut properties = sc_chain_spec::Properties::new();
		properties.insert("tokenSymbol".into(), "MYTH".into());
		properties.insert("tokenDecimals".into(), 18.into());
		properties.insert("ss58Format".into(), 333.into());
		properties.insert("isEthereum".into(), true.into());

		MainChainSpec::builder(
			mythical_mainnet::WASM_BINARY.expect("WASM binary was not build, please build it!"),
			Extensions {
				relay_chain: "polkadot".into(), // You MUST set this to the correct network!
				para_id: PARA_ID,
			},
		)
		.with_name("Mythical Mainnet")
		.with_id("mythical")
		.with_chain_type(ChainType::Live)
		.with_genesis_config_patch(mainnet_genesis(
			// initial collators.
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
				AccountId::from(hex!("ad49e6384184719D6ECC24DFEB61BF4D181138D8")),
				AccountId::from(hex!("90D157d5d32A01f7d518A804f821315f07DE2042")),
				AccountId::from(hex!("4FbF551aF1269DEba03C85Dbe990bA10EA28BCc6")),
			],
			// Example multisig sudo key configuration:
			// Configures 2/3 threshold multisig key
			// Note: For using this multisig key as a sudo key, each individual signatory must possess funds
			AccountId::from(hex!("4FbF551aF1269DEba03C85Dbe990bA10EA28BCc6")),
			PARA_ID.into(),
			Some(1_000_000_000 * MYTH),
		))
		.with_protocol_id("mythical")
		.with_properties(properties)
		.build()
	}

	fn mainnet_genesis(
		invulnerables: Vec<(AccountId, AuraId)>,
		endowed_accounts: Vec<AccountId>,
		root_key: AccountId,
		id: ParaId,
		total_issuance: Option<mythical_mainnet::Balance>,
	) -> serde_json::Value {
		use mythical_mainnet::EXISTENTIAL_DEPOSIT;
		//TODO: Define multisig root account
		//let alice = get_from_seed::<sr25519::Public>("Alice");
		//let bob = get_from_seed::<sr25519::Public>("Bob");

		let num_endowed_accounts = endowed_accounts.len();
		let balances = match total_issuance {
			Some(total_issuance) => {
				let balance_per_endowed = total_issuance
					.checked_div(num_endowed_accounts as mythical_mainnet::Balance)
					.unwrap_or(0 as mythical_mainnet::Balance);

				endowed_accounts.iter().cloned().map(|k| (k, balance_per_endowed)).collect()
			},
			None => vec![],
		};

		serde_json::json!({
				"balances": {
					"balances": balances
				},
				"parachainInfo": {
					"parachainId": id,
				},
				"collatorSelection": {
					"invulnerables": invulnerables.iter().cloned().map(|(acc, _)| acc).collect::<Vec<_>>(),
					"candidacyBond": EXISTENTIAL_DEPOSIT * 16,
				},
				"session": {
					"keys": invulnerables
						.into_iter()
						.map(|(acc, aura)| {
							(
								acc.clone(),                // account id
								acc,                        // validator id
								mainnet_session_keys(aura), // session keys
							)
						})
						.collect::<Vec<_>>(),
				},
				"sudo": { "key": Some(root_key) },
				"polkadotXcm": {
					"safeXcmVersion": Some(SAFE_XCM_VERSION),
				},
			}
		)
	}
}
