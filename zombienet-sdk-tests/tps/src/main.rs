use futures::{stream::FuturesUnordered, StreamExt};
use jsonrpsee_client_transport::ws::WsTransportClientBuilder;
use jsonrpsee_core::client::Client;
use parity_scale_codec::Decode;
use sp_core::{crypto::Pair as _, ecdsa};
use sp_runtime::traits::IdentifyAccount;
use std::{
	cell::RefCell,
	collections::HashMap,
	error::Error,
	sync::{atomic::AtomicU64, Arc},
	time::Duration,
	env,
};
use stps_config::eth::{AccountId20, EthereumSigner, MythicalConfig};
use subxt::{
	backend::legacy::LegacyBackend,
	config::DefaultExtrinsicParamsBuilder,
	dynamic::Value as TxValue,
	tx::{Signer, SubmittableTransaction},
	OnlineClient,
};
use zombienet_sdk::{NetworkConfigBuilder, NetworkConfigExt};

const NTRANS: usize = 4000;
const SENDER_SEED: &str = "//Sender";
const RECEIVER_SEED: &str = "//Receiver";
const ED: u128 = 10_000_000_000_000_000;
const SNAPS_BUCKET: &str = "https://storage.googleapis.com/project-mythical-tps";

type AccountInfo = frame_system::AccountInfo<u32, pallet_balances::AccountData<u128>>;
type TxOutput<C> = SubmittableTransaction<C, OnlineClient<C>>;

mod balance_transfer {
	use std::sync::atomic::{AtomicU64, Ordering::SeqCst};

	use super::*;
	use subxt::blocks::ExtrinsicDetails;

	pub fn tx_generator<'a>(
		client: &'a OnlineClient<MythicalConfig>,
		senders: impl Iterator<Item = &'a ecdsa::Pair> + 'a,
		receivers: impl Iterator<Item = &'a ecdsa::Pair> + 'a,
		nonces: &'a HashMap<AccountId20, AtomicU64>,
		_collections: &'a HashMap<AccountId20, [u64; 4]>,
		_nfts: &'a HashMap<AccountId20, Vec<([u64; 4], u128)>>,
	) -> impl Iterator<Item = TxOutput<MythicalConfig>> + 'a {
		let client = client.clone();
		senders.cloned().zip(receivers.cloned()).map(move |(sender, receiver)| {
			let nonce = nonces.get(&AccountId20::from(sender.clone())).expect("Nonces are known");
			let nonce = nonce.fetch_add(1, SeqCst);
			let tx_params =
				DefaultExtrinsicParamsBuilder::<MythicalConfig>::new().nonce(nonce).build();
			let tx_call = subxt::dynamic::tx(
				"Balances",
				"transfer_keep_alive",
				vec![TxValue::from_bytes(AccountId20::from(receiver).0), TxValue::u128(ED)],
			);
			client
				.tx()
				.create_partial_offline(&tx_call, tx_params.into())
				.unwrap()
				.sign(&EthereumSigner::from(sender))
		})
	}

	pub fn check_extrinsic(
		ex: &ExtrinsicDetails<MythicalConfig, OnlineClient<MythicalConfig>>,
	) -> bool {
		match (ex.pallet_name().unwrap(), ex.variant_name().unwrap()) {
			("Balances", "transfer_keep_alive") => true,
			_ => false,
		}
	}
}

mod nft_mint {
	use super::*;
	use std::sync::atomic::{AtomicU64, Ordering::SeqCst};
	use subxt::blocks::ExtrinsicDetails;

	pub fn tx_generator<'a>(
		client: &'a OnlineClient<MythicalConfig>,
		senders: impl Iterator<Item = &'a ecdsa::Pair> + 'a,
		_receivers: impl Iterator<Item = &'a ecdsa::Pair> + 'a,
		nonces: &'a HashMap<AccountId20, AtomicU64>,
		collections: &'a HashMap<AccountId20, [u64; 4]>,
		_nfts: &'a HashMap<AccountId20, Vec<([u64; 4], u128)>>,
	) -> impl Iterator<Item = TxOutput<MythicalConfig>> + 'a {
		let client = client.clone();
		senders.cloned().map(move |sender| {
			let nonce = nonces.get(&AccountId20::from(sender.clone())).expect("Nonces are known");
			let nonce = nonce.fetch_add(1, SeqCst);
			let tx_params =
				DefaultExtrinsicParamsBuilder::<MythicalConfig>::new().nonce(nonce).build();
			let tx_call = subxt::dynamic::tx(
				"Nfts",
				"mint",
				vec![
					TxValue::unnamed_composite(
						collections
							.get(&AccountId20::from(sender.clone()))
							.expect("Collections are known")
							.clone()
							.into_iter()
							.map(Into::into),
					),
					TxValue::unnamed_variant("None", vec![]),
					TxValue::from_bytes(&EthereumSigner::from(sender.clone()).into_account().0),
					TxValue::unnamed_variant("None", vec![]),
				],
			);
			client
				.tx()
				.create_partial_offline(&tx_call, tx_params.into())
				.unwrap()
				.sign(&EthereumSigner::from(sender))
		})
	}

	pub fn check_extrinsic(
		ex: &ExtrinsicDetails<MythicalConfig, OnlineClient<MythicalConfig>>,
	) -> bool {
		match (ex.pallet_name().unwrap(), ex.variant_name().unwrap()) {
			("Nfts", "mint") => true,
			_ => false,
		}
	}
}

mod nft_transfer {
	use super::*;
	use std::sync::atomic::{AtomicU64, Ordering::SeqCst};
	use subxt::blocks::ExtrinsicDetails;

	pub fn tx_generator<'a>(
		client: &'a OnlineClient<MythicalConfig>,
		senders: impl Iterator<Item = &'a ecdsa::Pair> + 'a,
		receivers: impl Iterator<Item = &'a ecdsa::Pair> + 'a,
		nonces: &'a HashMap<AccountId20, AtomicU64>,
		_collections: &'a HashMap<AccountId20, [u64; 4]>,
		nfts: &'a HashMap<AccountId20, Vec<([u64; 4], u128)>>,
	) -> impl Iterator<Item = TxOutput<MythicalConfig>> + 'a {
		let client = client.clone();
		senders.cloned().zip(receivers.cloned()).map(move |(sender, receiver)| {
			let nonce = nonces.get(&AccountId20::from(sender.clone())).expect("Nonces are known");
			let nonce = nonce.fetch_add(1, SeqCst);
			let (coll, nft_id) =
				nfts.get(&AccountId20::from(sender.clone())).expect("NFTs are known")[1];
			let tx_params =
				DefaultExtrinsicParamsBuilder::<MythicalConfig>::new().nonce(nonce).build();
			let tx_call = subxt::dynamic::tx(
				"Nfts",
				"transfer",
				vec![
					TxValue::unnamed_composite(coll.clone().into_iter().map(Into::into)),
					TxValue::u128(nft_id),
					TxValue::from_bytes(&EthereumSigner::from(receiver).into_account().0),
				],
			);
			client
				.tx()
				.create_partial_offline(&tx_call, tx_params.into())
				.unwrap()
				.sign(&EthereumSigner::from(sender))
		})
	}

	pub fn check_extrinsic(
		ex: &ExtrinsicDetails<MythicalConfig, OnlineClient<MythicalConfig>>,
	) -> bool {
		match (ex.pallet_name().unwrap(), ex.variant_name().unwrap()) {
			("Nfts", "transfer") => true,
			_ => false,
		}
	}
}

mod marketplace_order_bid {
	use super::*;
	use parity_scale_codec::Encode;
	use sp_core::U256;
	use std::sync::atomic::{AtomicU64, Ordering::SeqCst};
	use subxt::blocks::ExtrinsicDetails;

	pub fn tx_generator<'a>(
		client: &'a OnlineClient<MythicalConfig>,
		senders: impl Iterator<Item = &'a ecdsa::Pair> + 'a,
		receivers: impl Iterator<Item = &'a ecdsa::Pair> + 'a,
		nonces: &'a HashMap<AccountId20, AtomicU64>,
		_collections: &'a HashMap<AccountId20, [u64; 4]>,
		nfts: &'a HashMap<AccountId20, Vec<([u64; 4], u128)>>,
	) -> impl Iterator<Item = TxOutput<MythicalConfig>> + 'a {
		let client = client.clone();
		use rand::distr::{Alphanumeric, SampleString};

		#[derive(Encode)]
		pub struct OrderMessage {
			pub collection: U256,
			pub item: u128,
			pub price: u128,
			pub expires_at: u64,
			pub fee: u128,
			pub escrow_agent: Option<AccountId20>,
			pub nonce: String,
		}

		let fee_signer = EthereumSigner::from(ecdsa::Pair::from_seed(
			&subxt_signer::eth::dev::faith().secret_key(),
		));

		senders.cloned().zip(receivers.cloned()).map(move |(sender, receiver)| {
			let order_nonce: String = Alphanumeric.sample_string(&mut rand::rng(), 9);
			let nft = nfts.get(&AccountId20::from(sender.clone())).expect("NFTs are known")[0];

			let order_msg = OrderMessage {
				collection: U256(nft.0),
				item: nft.1,
				price: 1u128,
				expires_at: u64::MAX,
				fee: 1u128,
				escrow_agent: None,
				nonce: order_nonce.clone(),
			};
			let order_bytes = order_msg.encode();
			let signature = fee_signer.sign(&order_bytes[..]);

			let nonce = nonces.get(&AccountId20::from(receiver.clone())).expect("Nonces are known");
			let nonce = nonce.fetch_add(1, SeqCst);
			let tx_params =
				DefaultExtrinsicParamsBuilder::<MythicalConfig>::new().nonce(nonce).build();
			let tx_call = subxt::dynamic::tx(
				"Marketplace",
				"create_order",
				vec![
					(
						"order",
						TxValue::named_composite(vec![
							("order_type", TxValue::unnamed_variant("Bid", vec![])),
							(
								"collection",
								TxValue::unnamed_composite(
									nft.0.clone().into_iter().map(Into::into),
								),
							),
							("item", TxValue::u128(nft.1)),
							("price", TxValue::u128(1u128)),
							("expires_at", TxValue::primitive(u64::MAX.into())),
							("fee", TxValue::u128(1u128)),
							("escrow_agent", TxValue::unnamed_variant("None", vec![])),
							(
								"signature_data",
								TxValue::named_composite(vec![
									("signature", TxValue::from_bytes(&signature)),
									("nonce", TxValue::from_bytes(Vec::from(order_nonce))),
								]),
							),
						]),
					),
					("execution", TxValue::unnamed_variant("Force", vec![])),
				],
			);
			client
				.tx()
				.create_partial_offline(&tx_call, tx_params.into())
				.unwrap()
				.sign(&EthereumSigner::from(receiver))
		})
	}

	pub fn check_extrinsic(
		ex: &ExtrinsicDetails<MythicalConfig, OnlineClient<MythicalConfig>>,
	) -> bool {
		match (ex.pallet_name().unwrap(), ex.variant_name().unwrap()) {
			("Marketplace", "create_order") => ex.field_bytes()[0] == 0x01,
			_ => false,
		}
	}
}

async fn get_nonce(client: &OnlineClient<MythicalConfig>, account: AccountId20) -> u64 {
	let account_state_storage_addr = subxt::dynamic::storage(
		"System",
		"Account",
		vec![subxt::dynamic::Value::from_bytes(account.0)],
	);
	let account_state_enc = client
		.storage()
		.at_latest()
		.await
		.expect("Storage API available")
		.fetch(&account_state_storage_addr)
		.await
		.expect("Account status fetched")
		.expect("Account exists")
		.into_encoded();
	let account_state: AccountInfo =
		Decode::decode(&mut &account_state_enc[..]).expect("Account state decodes successfuly");
	account_state.nonce as u64
}

async fn get_first_collection_id(
	api: &OnlineClient<MythicalConfig>,
	account: AccountId20,
) -> [u64; 4] {
	let collection_id_storage_addr = subxt::dynamic::storage(
		"Nfts",
		"CollectionAccount",
		vec![subxt::dynamic::Value::from_bytes(account.0)],
	);
	let metadata = api.metadata();
	let address_bytes =
		subxt_core::storage::get_address_bytes(&collection_id_storage_addr, &metadata).unwrap();
	let address_len = address_bytes.len();
	// println!("Collection ID storage address: {:?}", address_bytes.clone());
	let mut coll_keys_stream = api
		.storage()
		.at_latest()
		.await
		.expect("Storage API available")
		.fetch_raw_keys(address_bytes)
		.await
		.expect("Collection keys fetched");
	let full_key = coll_keys_stream
		.next()
		.await
		.expect("Collection key fetched")
		.expect("Collection key exists");

	// FIXME: Dirty!
	assert_eq!(full_key.len(), address_len + 16 + 32); // 16 bytes of hash followed by 32 bytes of U256
	<[u64; 4]>::decode(&mut &full_key[address_len + 16..])
		.expect("Collection ID bytes should decode to [u64; 4]")
}

async fn get_nfts(
	api: &OnlineClient<MythicalConfig>,
	account: AccountId20,
) -> Vec<([u64; 4], u128)> {
	let collection_id_storage_addr = subxt::dynamic::storage(
		"Nfts",
		"Account",
		vec![subxt::dynamic::Value::from_bytes(account.0)],
	);
	let metadata = api.metadata();
	let address_bytes =
		subxt_core::storage::get_address_bytes(&collection_id_storage_addr, &metadata).unwrap();
	let address_len = address_bytes.len();
	let coll_keys_stream = api
		.storage()
		.at_latest()
		.await
		.expect("Storage API available")
		.fetch_raw_keys(address_bytes)
		.await
		.expect("NFT account keys fetched");

	// FIXME: Dirty!
	coll_keys_stream
		.take(2)
		.map(|k| {
			let full_key = k.expect("NFT account key fetched");
			assert_eq!(full_key.len(), address_len + 16 + 32 + 16 + 16); // 16 bytes of hash followed by 32 bytes of U256; then 16 bytes of hash followed by 16
																// bytes of u128
			let collection_id = <[u64; 4]>::decode(&mut &full_key[address_len + 16..])
				.expect("Collection ID bytes should decode to [u64; 4]");
			let nft_id = <u128>::decode(&mut &full_key[address_len + 16 + 32 + 16..])
				.expect("NFT ID bytes should decode to u128");
			(collection_id, nft_id)
		})
		.collect::<Vec<_>>()
		.await
}

async fn block_subscriber(
	api: OnlineClient<MythicalConfig>,
	ntrans: usize,
) -> Result<(), subxt::Error> {
	let mut blocks_sub = api.blocks().subscribe_finalized().await?;

	let mut total_ntrans = 0;
	let mut counters = HashMap::new();
	log::debug!("Starting chain watcher");
	while let Some(block) = blocks_sub.next().await {
		let block = block?;
		let blocknum = block.number();

		for ex in block.extrinsics().await?.iter() {
			if balance_transfer::check_extrinsic(&ex) {
				*counters
					.entry("balance_transfer")
					.or_insert_with(HashMap::new)
					.entry(blocknum)
					.or_insert(0) += 1;
				total_ntrans += 1;
			}
			if nft_mint::check_extrinsic(&ex) {
				*counters
					.entry("nft_mint")
					.or_insert_with(HashMap::new)
					.entry(blocknum)
					.or_insert(0) += 1;
				total_ntrans += 1;
			}
			if nft_transfer::check_extrinsic(&ex) {
				*counters
					.entry("nft_transfer")
					.or_insert_with(HashMap::new)
					.entry(blocknum)
					.or_insert(0) += 1;
				total_ntrans += 1;
			}
			if marketplace_order_bid::check_extrinsic(&ex) {
				*counters
					.entry("marketplace_order_bid")
					.or_insert_with(HashMap::new)
					.entry(blocknum)
					.or_insert(0) += 1;
				total_ntrans += 1;
			}
		}

		if total_ntrans >= ntrans {
			log::info!("{:?}", counters);
			break;
		}
	}
	Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
	env_logger::init_from_env(
		env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "info"),
	);

	let snaps_server = if let Ok(url) = env::var("ZOMBIE_SNAPS") {
		url
	} else {
		SNAPS_BUCKET.to_string()
	};

	let network = NetworkConfigBuilder::new().with_relaychain(|r| {
		r.with_chain("paseo-local")
			.with_chain_spec_path("chainspec/paseo-local.json")
			.with_default_command("polkadot")
			.with_node(|node| node.with_name("alice").with_db_snapshot(format!("{snaps_server}/alice.tgz").as_str()))
			.with_node(|node| node.with_name("bob").with_db_snapshot(format!("{snaps_server}/bob.tgz").as_str()))
	})

	// Always use a directory with enough space! (>25 Gb)
	.with_global_settings(|s| s.with_base_dir("/tmp/zn"))

	.with_parachain(|p| {
			p.with_id(3369).evm_based(true)
				.with_chain_spec_path("chainspec/local-v_paseo-local-3369.json")
				.with_default_command("mythos-node")
				.with_collator(|n| n.with_name("muse-collator01").with_db_snapshot(format!("{snaps_server}/muse-collator01.tgz").as_str()).with_args(
					vec![
						"--rpc-max-connections", "10000",
						"--rpc-max-subscriptions-per-connection", "65536",
						"--pool-type", "fork-aware",
						"--pool-limit", "500000",
						"--pool-kbytes", "2048000",
					].into_iter().map(Into::into).collect()
				))
				.with_collator(|n| n.with_name("muse-collator02").with_db_snapshot(format!("{snaps_server}/muse-collator02.tgz").as_str()))
		});

	let network = network.build().unwrap();
	let network = network.spawn_native().await?;

	let node = network.get_node("muse-collator01").unwrap();

	log::info!("Got node: {}", node.ws_uri());

	let current_block_ref = RefCell::new(0.0);
	node.wait_metric("block_height{status=\"best\"}", |v| {
		*current_block_ref.borrow_mut() = v;
		v > 0.0
	})
	.await?;

	let current_block = *current_block_ref.borrow();

	log::info!("At block: {}", current_block);

	node.wait_metric("block_height{status=\"best\"}", |v| v > current_block).await?;

	log::info!("Block production detected");

	let node_url = reqwest::Url::parse(&node.ws_uri())?;
	log::info!("Node URL: {}", node_url);
	let (node_sender, node_receiver) = {
		let mut last_error = None;
		let mut result = None;
		const MAX_RETRIES: u32 = 50;
		const RETRY_DELAY: Duration = Duration::from_secs(6);

		for attempt in 1..=MAX_RETRIES {
			match WsTransportClientBuilder::default().build(node_url.clone()).await {
				Ok(connection) => {
					log::info!("Successfully connected to node on attempt {}", attempt);
					result = Some(connection);
					break;
				},
				Err(e) => {
					last_error = Some(e);
					if attempt < MAX_RETRIES {
						log::warn!(
							"Connection attempt {} failed: {:?}. Retrying in {:?}...",
							attempt,
							last_error.as_ref().unwrap(),
							RETRY_DELAY
						);
						tokio::time::sleep(RETRY_DELAY).await;
					} else {
						log::error!("Connection failed after {} attempts.", MAX_RETRIES);
					}
				},
			}
		}

		result.ok_or_else(|| last_error.unwrap())?
	};
	log::info!("Node sender: {:?}", node_sender);
	let client = Client::builder()
		.request_timeout(Duration::from_secs(3600))
		.max_buffer_capacity_per_subscription(4096 * 1024)
		.max_concurrent_requests(2 * 1024 * 1024)
		.build_with_tokio(node_sender, node_receiver);
	log::info!("Client: {:?}", client);
	let backend = LegacyBackend::builder().build(client);
	log::info!("Backend built");
	let client = OnlineClient::from_backend(Arc::new(backend)).await?;
	log::info!("Online client built");

	let sender_accs: Vec<_> =
		funder::derive_accounts::<ecdsa::Pair>(NTRANS, SENDER_SEED.to_owned());
	let receiver_accs: Vec<_> =
		funder::derive_accounts::<ecdsa::Pair>(NTRANS, RECEIVER_SEED.to_owned());

	log::info!("Derived {} pairs of accounts", NTRANS);

	let futs = sender_accs
		.iter()
		.chain(receiver_accs.iter())
		.map(|a| {
			let account_id = EthereumSigner::from(a.clone()).account_id();
			let fapi = client.clone();
			async move {
				let nonce = get_nonce(&fapi, account_id).await;
				(account_id, AtomicU64::new(nonce))
			}
		})
		.collect::<FuturesUnordered<_>>();
	let noncemap = futs.collect::<Vec<_>>().await.into_iter().collect::<HashMap<_, _>>();

	log::info!("Got nonces");

	let futs = sender_accs
		.iter()
		.map(|a| {
			let account_id = EthereumSigner::from(a.clone()).account_id();
			let fapi = client.clone();
			async move {
				let coll_id = get_first_collection_id(&fapi, account_id).await;
				(account_id, coll_id)
			}
		})
		.collect::<FuturesUnordered<_>>();
	let collmap = futs.collect::<Vec<_>>().await.into_iter().collect::<HashMap<_, _>>();

	let futs = sender_accs
		.iter()
		.map(|a| {
			let account_id = EthereumSigner::from(a.clone()).account_id();
			let fapi = client.clone();
			async move {
				let nft_id = get_nfts(&fapi, account_id).await;
				(account_id, nft_id)
			}
		})
		.collect::<FuturesUnordered<_>>();
	let nftmap = futs.collect::<Vec<_>>().await.into_iter().collect::<HashMap<_, _>>();

	let txgroups = vec![
		balance_transfer::tx_generator(
			&client,
			sender_accs.iter(),
			receiver_accs.iter(),
			&noncemap,
			&collmap,
			&nftmap,
		)
		.collect::<Vec<_>>(),
		nft_mint::tx_generator(
			&client,
			sender_accs.iter(),
			receiver_accs.iter(),
			&noncemap,
			&collmap,
			&nftmap,
		)
		.collect::<Vec<_>>(),
		nft_transfer::tx_generator(
			&client,
			sender_accs.iter(),
			receiver_accs.iter(),
			&noncemap,
			&collmap,
			&nftmap,
		)
		.collect::<Vec<_>>(),
		marketplace_order_bid::tx_generator(
			&client,
			sender_accs.iter(),
			receiver_accs.iter(),
			&noncemap,
			&collmap,
			&nftmap,
		)
		.collect::<Vec<_>>(),
	];

	let fapi = client.clone();
	let ntrans = txgroups.iter().map(|tx| tx.len()).sum::<usize>();
	log::info!("Got {} transactions", ntrans);
	let subscriber = tokio::spawn(async move {
		match block_subscriber(fapi, ntrans).await {
			Ok(()) => {
				log::info!("Block subscriber exited");
			},
			Err(e) => {
				log::error!("Block subscriber exited with error: {:?}", e);
			},
		}
	});

	let mut submitted: Vec<_> = Vec::new();
	for txgroup in txgroups.into_iter() {
		let futs = txgroup.iter().map(|tx| tx.submit_and_watch()).collect::<FuturesUnordered<_>>();
		submitted.extend(
			futs.collect::<Vec<_>>()
				.await
				.into_iter()
				.collect::<Result<Vec<_>, _>>()
				.expect("All the transactions submitted successfully"),
		);
		log::info!("Submitted {} transactions", submitted.len());
		tokio::time::sleep(Duration::from_secs(12)).await;
	}
	log::info!("Submitted all the transactions");
	let waiting = submitted
		.into_iter()
		.map(|tx| tx.wait_for_finalized())
		.collect::<FuturesUnordered<_>>();
	let res = waiting
		.collect::<Vec<_>>()
		.await
		.into_iter()
		.collect::<Result<Vec<_>, _>>()
		.expect("All the transactions finalized successfully");
	log::info!("Finalized {} transactions", res.len());

	tokio::try_join!(subscriber).expect("Block subscriber joins successfully");
	log::info!("Test finished");

	Ok(())
}
