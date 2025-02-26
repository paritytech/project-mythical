use sc_chain_spec::{ChainSpecExtension, ChainSpecGroup};
use sc_service::ChainType;
use serde::{Deserialize, Serialize};

pub type GenericChainSpec = sc_service::GenericChainSpec<Extensions>;

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

pub mod testnet {
	const PARA_ID: u32 = 3369;

	use super::*;
	pub fn development_config() -> GenericChainSpec {
		// Give your base currency a unit name and decimal places
		let mut properties = sc_chain_spec::Properties::new();
		properties.insert("tokenSymbol".into(), "MUSE".into());
		properties.insert("tokenDecimals".into(), 18.into());
		properties.insert("ss58Format".into(), 29972.into());
		properties.insert("isEthereum".into(), true.into());

		GenericChainSpec::builder(
			testnet_runtime::WASM_BINARY.expect("WASM binary was not built, please build it!"),
			Extensions { relay_chain: "paseo-local".into(), para_id: PARA_ID },
		)
		.with_name("Development Muse Testnet")
		.with_id("testnet_muse_network_dev")
		.with_chain_type(ChainType::Development)
		.with_genesis_config_preset_name(sp_genesis_builder::DEV_RUNTIME_PRESET)
		.with_properties(properties)
		.build()
	}

	pub fn testnet_config() -> GenericChainSpec {
		// Give your base currency a unit name and decimal places
		let mut properties = sc_chain_spec::Properties::new();
		properties.insert("tokenSymbol".into(), "MUSE".into());
		properties.insert("tokenDecimals".into(), 18.into());
		properties.insert("ss58Format".into(), 29972.into());
		properties.insert("isEthereum".into(), true.into());

		GenericChainSpec::builder(
			testnet_runtime::WASM_BINARY.expect("WASM binary was not built, please build it!"),
			Extensions { relay_chain: "paseo".into(), para_id: PARA_ID },
		)
		.with_name("Muse Testnet")
		.with_id("muse")
		.with_chain_type(ChainType::Live)
		.with_genesis_config_preset_name(
			testnet_runtime::genesis_config_presets::MUSE_RUNTIME_PRESET,
		)
		.with_protocol_id("muse")
		.with_properties(properties)
		.build()
	}
}

pub mod mainnet {
	const PARA_ID: u32 = 3369;

	use super::*;
	pub fn development_config() -> GenericChainSpec {
		// Give your base currency a unit name and decimal places
		let mut properties = sc_chain_spec::Properties::new();
		properties.insert("tokenSymbol".into(), "MYTH".into());
		properties.insert("tokenDecimals".into(), 18.into());
		properties.insert("ss58Format".into(), 29972.into());
		properties.insert("isEthereum".into(), true.into());

		GenericChainSpec::builder(
			mainnet_runtime::WASM_BINARY.expect("WASM binary was not build, please build it!"),
			Extensions {
				relay_chain: "polkadot-local".into(), // You MUST set this to the correct network!
				para_id: PARA_ID,
			},
		)
		.with_name("Development MYTH Mainnet")
		.with_id("mainnet_mythos_network_dev")
		.with_chain_type(ChainType::Development)
		.with_genesis_config_preset_name(sp_genesis_builder::DEV_RUNTIME_PRESET)
		.with_properties(properties)
		.build()
	}

	pub fn _mainnet_config() -> GenericChainSpec {
		// Give your base currency a unit name and decimal places
		let mut properties = sc_chain_spec::Properties::new();
		properties.insert("tokenSymbol".into(), "MYTH".into());
		properties.insert("tokenDecimals".into(), 18.into());
		properties.insert("ss58Format".into(), 29972.into());
		properties.insert("isEthereum".into(), true.into());

		GenericChainSpec::builder(
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
		.with_genesis_config_preset_name(mainnet_runtime::genesis_config_presets::MYTHOS_RUNTIME_PRESET)
		.with_protocol_id("mythos")
		.with_properties(properties)
		.build()
	}
}
