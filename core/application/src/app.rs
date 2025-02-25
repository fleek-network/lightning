use std::marker::PhantomData;
use std::sync::{Arc, Mutex};

use affair::AsyncWorker;
use anyhow::{anyhow, Result};
use lightning_interfaces::prelude::*;
use lightning_interfaces::spawn_worker;
use lightning_interfaces::types::{ChainId, NodeInfo};
use tracing::{error, info};
use types::Genesis;

use crate::config::{ApplicationConfig, StorageConfig};
use crate::env::{ApplicationEnv, Env, UpdateWorker};
use crate::state::QueryRunner;
pub struct Application<C: NodeComponents> {
    env: Arc<tokio::sync::Mutex<ApplicationEnv>>,
    update_socket: Mutex<Option<ExecutionEngineSocket>>,
    query_runner: QueryRunner,
    _components: PhantomData<C>,
}

impl<C: NodeComponents> Application<C> {
    /// Create a new instance of the application layer using the provided configuration.
    fn init(
        config: &C::ConfigProviderInterface,
        blockstore: &C::BlockstoreInterface,
        fdi::Cloned(waiter): fdi::Cloned<ShutdownWaiter>,
    ) -> Result<Self> {
        let config = config.get::<Self>();
        if let StorageConfig::RocksDb = &config.storage {
            assert!(
                config.db_path.is_some(),
                "db_path must be specified for RocksDb backend"
            );
        }
        // 1. add time consesus
        // 2. add counter.
        // 3. add worker and send transactions that it was executed and response. ( worker will
        //    execute the job)
        let mut env = Env::new(&config, None).expect("Failed to initialize environment.");

        // Apply genesis if provided, if it hasn't been applied yet.
        if let Some(genesis) = config.genesis()? {
            env.apply_genesis_block(genesis)?;
        }

        let query_runner = env.query_runner();
        let env = Arc::new(tokio::sync::Mutex::new(env));
        let worker = UpdateWorker::<C>::new(env.clone(), blockstore.clone());
        let update_socket = spawn_worker!(worker, "APPLICATION", waiter, crucial);

        Ok(Self {
            env,
            query_runner,
            update_socket: Mutex::new(Some(update_socket)),
            _components: PhantomData,
        })
    }
}

impl<C: NodeComponents> ConfigConsumer for Application<C> {
    const KEY: &'static str = "application";

    type Config = ApplicationConfig;
}

impl<C: NodeComponents> fdi::BuildGraph for Application<C> {
    fn build_graph() -> fdi::DependencyGraph {
        fdi::DependencyGraph::new().with(Self::init)
    }
}

impl<C: NodeComponents> ApplicationInterface<C> for Application<C> {
    /// The type for the sync query executor.
    type SyncExecutor = QueryRunner;

    /// Returns a socket that should be used to submit transactions to be executed
    /// by the application layer.
    ///
    /// # Safety
    ///
    /// See the safety document for the [`ExecutionEngineSocket`].
    fn transaction_executor(&self) -> ExecutionEngineSocket {
        self.update_socket
            .lock()
            .unwrap()
            .take()
            .expect("Execution Engine Socket has already been taken")
    }

    /// Returns the instance of a sync query runner which can be used to run queries without
    /// blocking or awaiting. A naive (& blocking) implementation can achieve this by simply
    /// putting the entire application state in an `Arc<RwLock<T>>`, but that is not optimal
    /// and is the reason why we have `Atomo` to allow us to have the same kind of behavior
    /// without slowing down the system.
    fn sync_query(&self) -> Self::SyncExecutor {
        self.query_runner.clone()
    }

    async fn load_from_checkpoint(
        config: &ApplicationConfig,
        checkpoint: Vec<u8>,
        checkpoint_hash: [u8; 32],
    ) -> Result<()> {
        match ApplicationEnv::new(config, Some((checkpoint_hash, &checkpoint))) {
            Ok(mut env) => {
                info!("Successfully built database from checkpoint with hash {checkpoint_hash:?}");
                env.update_last_epoch_hash(checkpoint_hash)?;
                Ok(())
            },
            Err(e) => {
                error!("Failed to build app db from checkpoint: {e:?}");
                Err(anyhow!("Failed to build app db from checkpoint: {}", e))
            },
        }
    }

    fn get_chain_id(config: &ApplicationConfig) -> Result<ChainId> {
        Ok(config
            .genesis()?
            .ok_or(anyhow!("missing genesis"))?
            .chain_id)
    }

    fn get_genesis_committee(config: &ApplicationConfig) -> Result<Vec<NodeInfo>> {
        Ok(config
            .genesis()?
            .ok_or(anyhow!("missing genesis"))?
            .node_info
            .iter()
            .filter(|node| node.genesis_committee)
            .map(NodeInfo::from)
            .collect())
    }

    /// Resets the state tree by clearing it and rebuilding it from the full state.
    ///
    /// This method is unsafe because it acts directly on the underlying storage backend.
    fn reset_state_tree_unsafe(config: &ApplicationConfig) -> Result<()> {
        let mut env = ApplicationEnv::new(config, None)?;
        env.inner.reset_state_tree_unsafe()
    }

    /// Apply genesis block to the application state, if not already applied.
    async fn apply_genesis(&self, genesis: Genesis) -> Result<bool> {
        self.env.lock().await.apply_genesis_block(genesis)
    }
}
