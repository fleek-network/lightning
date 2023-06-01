use anyhow::Result;
use async_trait::async_trait;
use draco_interfaces::{
    application::{
        ApplicationInterface, ExecutionEngineSocket, QuerySocket, SyncQueryRunnerInterface,
    },
    common::WithStartAndShutdown,
    config::ConfigConsumer,
    types::NodeInfo,
};
use fleek_crypto::{ClientPublicKey, NodePublicKey};

use super::config::Config;

pub struct Application {}

#[async_trait]
impl WithStartAndShutdown for Application {
    /// Returns true if this system is running or not.
    fn is_running(&self) -> bool {
        todo!()
    }

    /// Start the system, should not do anything if the system is already
    /// started.
    async fn start(&self) {
        todo!()
    }

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
        todo!()
    }

    /// Returns a socket that should be used to submit transactions to be executed
    /// by the application layer.
    ///
    /// # Safety
    ///
    /// See the safety document for the [`ExecutionEngineSocket`].
    fn transaction_executor(&self) -> ExecutionEngineSocket {
        todo!()
    }

    /// Returns a socket that can be used to execute queries on the application layer. This
    /// socket can be passed to the *RPC* as an example.
    fn query_socket(&self) -> QuerySocket {
        todo!()
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

#[derive(Clone)]
pub struct QueryRunner {}

impl SyncQueryRunnerInterface for QueryRunner {
    /// Returns the latest balance associated with the given peer.
    fn get_balance(&self, client: &ClientPublicKey) -> u128 {
        todo!()
    }

    /// Returns the global reputation of a node.
    fn get_reputation(&self, node: &NodePublicKey) -> u128 {
        todo!()
    }

    /// Returns the relative score between two nodes, this score should measure how much two
    /// nodes `n1` and `n2` trust each other. Of course in real world a direct measurement
    /// between any two node might not exits, but there does exits a path from `n1` to `n2`
    /// which may cross any `node_i`, a page rank like algorithm is needed here to measure
    /// a relative score between two nodes.
    ///
    /// Existence of this data can allow future optimizations of the network topology.
    fn get_relative_score(&self, n1: &NodePublicKey, n2: &NodePublicKey) -> u128 {
        todo!()
    }

    /// Returns information about a single node.
    fn get_node_info(&self, id: &NodePublicKey) -> Option<NodeInfo> {
        todo!()
    }

    /// Returns a full copy of the entire node-registry, but only contains the nodes that
    /// are still a valid node and have enough stake.
    fn get_node_registry(&self) -> Vec<NodeInfo> {
        todo!()
    }

    /// Returns true if the node is a valid node in the network, with enough stake.
    fn is_valid_node(&self, id: &NodePublicKey) -> bool {
        todo!()
    }

    /// Returns the amount that is required to be a valid node in the network.
    fn get_staking_amount(&self) -> u128 {
        todo!()
    }

    /// Returns the randomness that was used to start the current epoch.
    fn get_epoch_randomness_seed(&self) -> &[u8; 32] {
        todo!()
    }

    /// Returns the committee members of the current epoch.
    fn get_committee_members(&self) -> Vec<NodePublicKey> {
        todo!()
    }
}
