use affair::{Executor, TokioSpawn};
use anyhow::Result;
use async_trait::async_trait;
use draco_interfaces::{
    application::{ApplicationInterface, ExecutionEngineSocket, QuerySocket},
    common::WithStartAndShutdown,
    config::ConfigConsumer,
};

use crate::{
    config::Config,
    env::{Env, QueryWorker, UpdateWorker},
    query_runner::QueryRunner,
};

pub struct Application {
    query_socket: QuerySocket,
    update_socket: ExecutionEngineSocket,
}

#[async_trait]
impl WithStartAndShutdown for Application {
    /// Returns true if this system is running or not.
    fn is_running(&self) -> bool {
        true
    }

    /// Start the system, should not do anything if the system is already
    /// started.
    async fn start(&self) {}

    /// Send the shutdown signal to the system.
    async fn shutdown(&self) {
        todo!()
    }
}

impl ConfigConsumer for Application {
    const KEY: &'static str = "application";

    type Config = Config;
}

#[async_trait]
impl ApplicationInterface for Application {
    /// The type for the sync query executor.
    type SyncExecutor = QueryRunner;

    /// Create a new instance of the application layer using the provided configuration.
    async fn init(_config: Self::Config) -> Result<Self> {
        let mut env = Env::new();
        env.genesis();
        Ok(Self {
            query_socket: TokioSpawn::spawn_async(QueryWorker::new(env.query())),
            update_socket: TokioSpawn::spawn(UpdateWorker::new(env)),
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

    /// Returns a socket that can be used to execute queries on the application layer. This
    /// socket can be passed to the *RPC* as an example.
    fn query_socket(&self) -> QuerySocket {
        self.query_socket.clone()
    }

    /// Returns the instance of a sync query runner which can be used to run queries without
    /// blocking or awaiting. A naive (& blocking) implementation can achieve this by simply
    /// putting the entire application state in an `Arc<RwLock<T>>`, but that is not optimal
    /// and is the reason why we have `Atomo` to allow us to have the same kind of behavior
    /// without slowing down the system.
    fn sync_query(&self) -> Self::SyncExecutor {
        todo!()
    }
}
