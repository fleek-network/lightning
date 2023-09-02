use std::marker::PhantomData;

use affair::{Executor, TokioSpawn};
use anyhow::Result;
use async_trait::async_trait;
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::{
    ApplicationInterface,
    ConfigConsumer,
    ExecutionEngineSocket,
    WithStartAndShutdown,
};
use log::info;

use crate::config::{Config, StorageConfig};
use crate::env::{Env, UpdateWorker};
use crate::query_runner::QueryRunner;
pub struct Application<C: Collection> {
    update_socket: ExecutionEngineSocket,
    query_runner: QueryRunner,
    collection: PhantomData<C>,
}

#[async_trait]
impl<C: Collection> WithStartAndShutdown for Application<C> {
    /// Returns true if this system is running or not.
    fn is_running(&self) -> bool {
        true
    }

    /// Start the system, should not do anything if the system is already
    /// started.
    async fn start(&self) {
        // No op because application is started in the init
    }

    /// Send the shutdown signal to the system.
    async fn shutdown(&self) {}
}

impl<C: Collection> ConfigConsumer for Application<C> {
    const KEY: &'static str = "application";

    type Config = Config;
}

#[async_trait]
impl<C: Collection> ApplicationInterface<C> for Application<C> {
    /// The type for the sync query executor.
    type SyncExecutor = QueryRunner;

    /// Create a new instance of the application layer using the provided configuration.
    #[allow(unused)]
    fn init(
        config: Self::Config,
        blockstore: C::BlockStoreInterface,
        blockstore_server: C::BlockStoreServerInterface,
    ) -> Result<Self> {
        if let StorageConfig::RocksDb = &config.storage {
            assert!(
                config.db_path.is_some(),
                "db_path must be specified for RocksDb backend"
            );
        }

        let mut env = Env::new(&config, None).expect("Failed to initialize environment.");

        if !env.genesis(&config) {
            info!("State already exists. Not loading genesis.");
        }

        Ok(Self {
            query_runner: env.query_runner(),
            update_socket: TokioSpawn::spawn_async(UpdateWorker::<C>::new(env, blockstore)),
            collection: PhantomData,
        })
    }

    /// Returns a socket that should be used to submit transactions to be executed
    /// by the application layer.
    ///
    /// # Safety
    ///
    /// See the safety document for the [`ExecutionEngineSocket`].
    fn transaction_executor(&self) -> ExecutionEngineSocket {
        self.update_socket.clone()
    }

    /// Returns the instance of a sync query runner which can be used to run queries without
    /// blocking or awaiting. A naive (& blocking) implementation can achieve this by simply
    /// putting the entire application state in an `Arc<RwLock<T>>`, but that is not optimal
    /// and is the reason why we have `Atomo` to allow us to have the same kind of behavior
    /// without slowing down the system.
    fn sync_query(&self) -> Self::SyncExecutor {
        self.query_runner.clone()
    }
}
