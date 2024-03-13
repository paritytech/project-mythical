use cumulus_primitives_core::ParaId;
use hex_literal::hex;
use runtime_common::{AccountId, AuraId, EthereumSignature};
use sc_chain_spec::{ChainSpecExtension, ChainSpecGroup};
use sc_service::ChainType;
use serde::{Deserialize, Serialize};
use sp_core::{crypto::UncheckedInto, ecdsa, Pair, Public};
use sp_runtime::traits::{IdentifyAccount, Verify};

use mainnet_runtime;
use testnet_runtime;

/// Specialized `ChainSpec` for the normal parachain runtime.
pub type MainChainSpec =
	sc_service::GenericChainSpec<mainnet_runtime::RuntimeGenesisConfig, Extensions>;

/// Specialized `ChainSpec` for the development parachain runtime.
pub type TestnetChainSpec =
	sc_service::GenericChainSpec<testnet_runtime::RuntimeGenesisConfig, Extensions>;

/// The default XCM version to set in genesis config.
const SAFE_XCM_VERSION: u32 = xcm::prelude::XCM_VERSION;

const LOCAL_PARA_ID: u32 = 2000;

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
pub fn mainnet_session_keys(keys: AuraId) -> mainnet_runtime::SessionKeys {
	mainnet_runtime::SessionKeys { aura: keys }
}

pub fn testnet_session_keys(keys: AuraId) -> testnet_runtime::SessionKeys {
	testnet_runtime::SessionKeys { aura: keys }
}

pub mod testnet {
	const PARA_ID: u32 = 201804;
	use testnet_runtime::MUSE;

	use super::*;
	pub fn development_config() -> TestnetChainSpec {
		// Give your base currency a unit name and decimal places
		let mut properties = sc_chain_spec::Properties::new();
		properties.insert("tokenSymbol".into(), "MUSE".into());
		properties.insert("tokenDecimals".into(), 18.into());
		properties.insert("ss58Format".into(), 29972.into());
		properties.insert("isEthereum".into(), true.into());

		let balance_per_account = (1_000_000_000 * MUSE).saturating_div(6);

		TestnetChainSpec::builder(
			testnet_runtime::WASM_BINARY.expect("WASM binary was not built, please build it!"),
			Extensions {
				relay_chain: "rococo-local".into(), // You MUST set this to the correct network!
				para_id: PARA_ID,
			},
		)
		// Name
		.with_name("Development Muse Testnet")
		// ID
		.with_id("testnet_muse_network_dev")
		.with_chain_type(ChainType::Development)
		.with_genesis_config_patch(testnet_genesis(
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
			AccountId::from(hex!("f24FF3a9CF04c71Dbc94D0b566f7A27B94566cac")),
			LOCAL_PARA_ID.into(),
		))
		.with_properties(properties)
		.build()
	}

	pub fn testnet_config() -> TestnetChainSpec {
		// Give your base currency a unit name and decimal places
		let mut properties = sc_chain_spec::Properties::new();
		properties.insert("tokenSymbol".into(), "MUSE".into());
		properties.insert("tokenDecimals".into(), 18.into());
		properties.insert("ss58Format".into(), 29972.into());
		properties.insert("isEthereum".into(), true.into());

		let balance_per_account = (1_000_000_000 * MUSE).saturating_div(3);

		TestnetChainSpec::builder(
			testnet_runtime::WASM_BINARY.expect("WASM binary was not built, please build it!"),
			Extensions {
				relay_chain: "rococo".into(), // You MUST set this to the correct network!
				para_id: PARA_ID,
			},
		)
		.with_name("Muse Testnet")
		.with_id("muse")
		.with_chain_type(ChainType::Live)
		.with_genesis_config_patch(testnet_genesis(
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
				(
					AccountId::from(hex!("ad49e6384184719D6ECC24DFEB61BF4D181138D8")),
					balance_per_account,
				),
				(
					AccountId::from(hex!("90D157d5d32A01f7d518A804f821315f07DE2042")),
					balance_per_account,
				),
				(
					AccountId::from(hex!("4FbF551aF1269DEba03C85Dbe990bA10EA28BCc6")),
					balance_per_account,
				),
			],
			AccountId::from(hex!("4FbF551aF1269DEba03C85Dbe990bA10EA28BCc6")),
			PARA_ID.into(),
		))
		.with_protocol_id("muse")
		.with_properties(properties)
		.build()
	}

	fn testnet_genesis(
		invulnerables: Vec<(AccountId, AuraId)>,
		endowed_accounts: Vec<(AccountId, testnet_runtime::Balance)>,
		root_key: AccountId,
		id: ParaId,
	) -> serde_json::Value {
		use testnet_runtime::EXISTENTIAL_DEPOSIT;

		serde_json::json!({
				"balances": {
					"balances": endowed_accounts,
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
								testnet_session_keys(aura), // session keys
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
	const PARA_ID: u32 = 3369;
	use mainnet_runtime::MYTH;

	use super::*;
	pub fn development_config() -> MainChainSpec {
		// Give your base currency a unit name and decimal places
		let mut properties = sc_chain_spec::Properties::new();
		properties.insert("tokenSymbol".into(), "MYTH".into());
		properties.insert("tokenDecimals".into(), 18.into());
		properties.insert("ss58Format".into(), 29972.into());
		properties.insert("isEthereum".into(), true.into());

		let balance_per_account = (1_000_000_000 * MYTH).saturating_div(6);

		MainChainSpec::builder(
			mainnet_runtime::WASM_BINARY.expect("WASM binary was not build, please build it!"),
			Extensions {
				relay_chain: "polkadot-local".into(), // You MUST set this to the correct network! TODO: Change to polkadot-local
				para_id: LOCAL_PARA_ID,
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
			AccountId::from(hex!("f24FF3a9CF04c71Dbc94D0b566f7A27B94566cac")),
			LOCAL_PARA_ID.into(),
		))
		.with_properties(properties)
		.build()
	}

	pub fn mainnet_config() -> MainChainSpec {
		// Give your base currency a unit name and decimal places
		let mut properties = sc_chain_spec::Properties::new();
		properties.insert("tokenSymbol".into(), "MYTH".into());
		properties.insert("tokenDecimals".into(), 18.into());
		properties.insert("ss58Format".into(), 29972.into());
		properties.insert("isEthereum".into(), true.into());

		MainChainSpec::builder(
			mainnet_runtime::WASM_BINARY.expect("WASM binary was not build, please build it!"),
			Extensions {
				relay_chain: "polkadot".into(), // You MUST set this to the correct network!
				para_id: PARA_ID,
			},
		)
		.with_name("Mythos")
		.with_id("mythos")
		.with_chain_type(ChainType::Live)
		.with_boot_nodes(vec![
			"/dns/polkadot-mythos-connect-0.polkadot.io/tcp/30333/p2p/12D3KooWJ3zJMjcReodmHx5KLm9LwbFtLvScncqj89UX5j8VYMUf"
				.parse()
				.expect("MultiaddrWithPeerId"),
			"/dns/polkadot-mythos-connect-0.polkadot.io/tcp/443/wss/p2p/12D3KooWJ3zJMjcReodmHx5KLm9LwbFtLvScncqj89UX5j8VYMUf"
				.parse()
				.expect("MultiaddrWithPeerId"),
			"/dns/polkadot-mythos-connect-1.polkadot.io/tcp/30333/p2p/12D3KooWLin9rPs8irgJZgFTab6nhQjFSVp6xYTPTrLGrbjZypeu"
				.parse()
				.expect("MultiaddrWithPeerId"),
			"/dns/polkadot-mythos-connect-1.polkadot.io/tcp/443/wss/p2p/12D3KooWLin9rPs8irgJZgFTab6nhQjFSVp6xYTPTrLGrbjZypeu"
				.parse()
				.expect("MultiaddrWithPeerId"),
		])
		.with_genesis_config_patch(mainnet_genesis(
			// initial collators.
			vec![
				(
					hex!("d609c361de761b4bf8ba1ae4f8e436e74e1324b0a9eac08b34e31413bbd3f27f").into(),
					hex!("d609c361de761b4bf8ba1ae4f8e436e74e1324b0a9eac08b34e31413bbd3f27f")
						.unchecked_into(),
				),
				(
					hex!("8abe92437bf6690bc8f75cea612a5898cd2823c23681b346f776337660316979").into(),
					hex!("8abe92437bf6690bc8f75cea612a5898cd2823c23681b346f776337660316979")
						.unchecked_into(),
				),
				(
					hex!("86360126eb30d60c9232206ba78a9fafb2322958bb3a021fa88ba09dfc753802").into(),
					hex!("86360126eb30d60c9232206ba78a9fafb2322958bb3a021fa88ba09dfc753802")
						.unchecked_into(),
				),
				(
					hex!("b0909f6832d2f5120b874b3e1cbe1b72fb5ccdbc268ba79bebdd8e71ab41e334").into(),
					hex!("b0909f6832d2f5120b874b3e1cbe1b72fb5ccdbc268ba79bebdd8e71ab41e334")
						.unchecked_into(),
				),
			],
			vec![
				(
					AccountId::from(hex!("742c722892976C23A3919ADC7A4B562169B91E41")),
					1_000 * MYTH
				),
				(
					AccountId::from(hex!("f476dA221b07135b106d923b8884b76b09982B4F")),
					150_000_000 * MYTH,
				),
			],
			AccountId::from(hex!("742c722892976C23A3919ADC7A4B562169B91E41")),
			PARA_ID.into(),
		))
		.with_protocol_id("mythos")
		.with_properties(properties)
		.build()
	}

	fn mainnet_genesis(
		invulnerables: Vec<(AccountId, AuraId)>,
		endowed_accounts: Vec<(AccountId, mainnet_runtime::Balance)>,
		root_key: AccountId,
		id: ParaId,
	) -> serde_json::Value {
		use mainnet_runtime::EXISTENTIAL_DEPOSIT;

		serde_json::json!({
				"balances": {
					"balances": endowed_accounts,
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
