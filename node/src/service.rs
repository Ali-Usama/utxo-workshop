//! Service and ServiceFactory implementation. Specialized wrapper over substrate service.

use std::{sync::Arc, time::Duration};
use sc_client_api::ExecutorProvider;
use utxo_runtime::{self, opaque::Block, RuntimeApi};
use sp_runtime::MultiSigner;
use sc_service::{error::Error as ServiceError, Configuration, PartialComponents, TaskManager, TFullCallExecutor};
use sp_inherents::CreateInherentDataProviders;
use sha3pow::*;
use core::clone::Clone;
use sc_consensus::{LongestChain, import_queue::BasicQueue};
use sc_executor::{NativeElseWasmExecutor};
use sp_core::{Encode, U256};
use sc_telemetry::{Telemetry, TelemetryWorker};
use sc_consensus_pow::{PowBlockImport};
use sp_api::TransactionFor;
use sp_consensus::CanAuthorWithNativeVersion;
use sp_runtime::traits::IdentifyAccount;


pub type Executor = NativeElseWasmExecutor<ExecutorDispatch>;

// Our native executor instance.
pub struct ExecutorDispatch;

impl sc_executor::NativeExecutionDispatch for ExecutorDispatch {
	/// Only enable the benchmarking host functions when we actually want to benchmark.
	#[cfg(feature = "runtime-benchmarks")]
	type ExtendHostFunctions = frame_benchmarking::benchmarking::HostFunctions;
	/// Otherwise we only use the default Substrate host functions.
	#[cfg(not(feature = "runtime-benchmarks"))]
	type ExtendHostFunctions = ();

	fn dispatch(method: &str, data: &[u8]) -> Option<Vec<u8>> {
		utxo_runtime::api::dispatch(method, data)
	}

	fn native_version() -> sc_executor::NativeVersion {
		utxo_runtime::native_version()
	}
}

pub(crate) type FullClient = sc_service::TFullClient<Block, RuntimeApi, Executor>;
type FullBackend = sc_service::TFullBackend<Block>;
type FullSelectChain = LongestChain<FullBackend, Block>;

/// Returns most parts of a service. Not enough to run a full chain,
/// But enough to perform chain operations like purge-chain
pub fn new_partial(config: &Configuration) -> Result<
	PartialComponents<
		FullClient, FullBackend, FullSelectChain,
		sc_consensus::DefaultImportQueue<Block, FullClient>,
		sc_transaction_pool::FullPool<Block, FullClient>,
		(PowBlockImport<
			Block,
			Arc<FullClient>,
			FullClient,
			FullSelectChain,
			MinimalSha3Algorithm,
			CanAuthorWithNativeVersion<TFullCallExecutor<Block, Executor>>,
			Box<
				dyn CreateInherentDataProviders<
					Block,
					(),
					InherentDataProviders=sp_timestamp::InherentDataProvider,
				>,
			>,
		>,
		 Option<Telemetry>)
	>,
	ServiceError> {
	// let inherent_data_providers = build_inherent_data_providers(sr25519_public_key)?;

	if config.keystore_remote.is_some() {
		return Err(ServiceError::Other("Remote Keystores are not supported.".into()));
	}

	let executor = NativeElseWasmExecutor::<ExecutorDispatch>::new(
		config.wasm_method,
		config.default_heap_pages,
		config.max_runtime_instances,
		config.runtime_cache_size,
	);

	let telemetry = config
		.telemetry_endpoints
		.clone()
		.filter(|x| !x.is_empty())
		.map(|endpoints| -> Result<_, sc_telemetry::Error> {
			let worker = TelemetryWorker::new(16)?;
			let telemetry = worker.handle().new_telemetry(endpoints);
			Ok((worker, telemetry))
		})
		.transpose()?;

	let (client, backend, keystore_container, task_manager) =
		sc_service::new_full_parts::<Block, RuntimeApi, Executor>(&config,
																  telemetry.as_ref().map(|(_, telemetry)| telemetry.handle()),
																  executor)?;
	let client = Arc::new(client);
	let telemetry = telemetry.map(|(worker, telemetry)| {
		task_manager.spawn_handle().spawn("telemetry", None, worker.run());
		telemetry
	});

	let select_chain = LongestChain::new(backend.clone());

	let transaction_pool = sc_transaction_pool::BasicPool::new_full(config.transaction_pool.clone(),
																	config.role.is_authority().into(),
																	config.prometheus_registry(),
																	task_manager.spawn_essential_handle(),
																	client.clone());


	let can_author_with =
		CanAuthorWithNativeVersion::new(client.executor().clone());

	// let (grandpa_block_import, grandpa_link) = sc_finality_grandpa::block_import(
	// 	client.clone(),
	// 	&(client.clone() as Arc<_>),
	// 	select_chain.clone(),
	// 	telemetry.as_ref().map(|x| x.handle()),
	// )?;

	let pow_block_import = PowBlockImport::new(
		client.clone(),
		client.clone(),
		MinimalSha3Algorithm,
		0, // check inherents starting at block 0
		select_chain.clone(),
		Box::new(move |_, ()| async move {
			let provider = sp_timestamp::InherentDataProvider::from_system_time();
			Ok(provider)
		})
			as Box<
			dyn CreateInherentDataProviders<
				Block,
				(),
				InherentDataProviders=sp_timestamp::InherentDataProvider,
			>,
		>,
		can_author_with,
	);

	let import_queue = sc_consensus_pow::import_queue(
		Box::new(pow_block_import.clone()),
		None,
		MinimalSha3Algorithm,
		&task_manager.spawn_essential_handle(),
		config.prometheus_registry(),
	)?;

	Ok(PartialComponents {
		client,
		backend,
		task_manager,
		import_queue,
		keystore_container,
		select_chain,
		transaction_pool,
		other: (pow_block_import, telemetry),
	})
}

/// Builds a new service for a full client.
pub fn new_full(config: Configuration, sr25519_public_key: usize) -> Result<TaskManager, ServiceError> {
	let PartialComponents {
		client, backend, mut task_manager, import_queue,
		mut keystore_container,
		select_chain: _, transaction_pool,
		other: (pow_block_import, mut telemetry)
	} = new_partial(&config)?;

	let (network, system_rpc_tx, network_starter) =
		sc_service::build_network(sc_service::BuildNetworkParams {
			config: &config,
			client: client.clone(),
			transaction_pool: transaction_pool.clone(),
			spawn_handle: task_manager.spawn_handle(),
			import_queue,
			block_announce_validator_builder: None,
			warp_sync: None,
		})?;

	let role = config.role.clone();
	let prometheus_registry = config.prometheus_registry().cloned();
	// let telemetry_connection_sinks = sc_service::TelemetryConnectionSinks::default();
	let rpc_extensions_builder = {
		let client = client.clone();
		let pool = transaction_pool.clone();

		Box::new(move |deny_unsafe, _| {
			let deps =
				crate::rpc::FullDeps { client: client.clone(), pool: pool.clone(), deny_unsafe };
			crate::rpc::create_full(deps).map_err(Into::into)
		})
	};

	sc_service::spawn_tasks(sc_service::SpawnTasksParams {
		network: network.clone(),
		client: client.clone(),
		keystore: keystore_container.sync_keystore(),
		task_manager: &mut task_manager,
		transaction_pool: transaction_pool.clone(),
		backend: backend.clone(),
		system_rpc_tx,
		config,
		rpc_builder: rpc_extensions_builder,
		telemetry: telemetry.as_mut(),
	})?;


	if role.is_authority() {
		let proposer = sc_basic_authorship::ProposerFactory::new(task_manager.spawn_handle(), client.clone(),
																 transaction_pool,
																 prometheus_registry.as_ref(),
																 telemetry.as_ref().map(|x| x.handle()));

		// The number of rounds of mining to try in a single call
		let rounds = 500;

		let can_author_with =
			sp_consensus::CanAuthorWithNativeVersion::new(client.executor().clone());
		let select_chain = sc_consensus::LongestChain::new(backend.clone());

		let address = MultiSigner::from(sp_keyring::Sr25519Keyring::Alice.public())
			.into_account()
			.encode();

		sc_consensus_pow::start_mining_worker(Box::new(pow_block_import),
											  client,
											  select_chain,
											  MinimalSha3Algorithm,
											  proposer, network.clone(), network,
											  Some(address),
											  move |_, ()| async move {
												  let provider = sp_timestamp::InherentDataProvider::from_system_time();
												  Ok(provider)
											  },
											  std::time::Duration::new(2, 0),
											  std::time::Duration::new(10, 0),
											  can_author_with.clone());
	}

	network_starter.start_network();
	Ok(task_manager)
}
