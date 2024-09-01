use anyhow::Result;
use futures::future::join_all;
use lightning_interfaces::ApplicationInterface;
use ready::ReadyWaiter;
use tempfile::tempdir;

use super::{TestGenesisBuilder, TestNetwork, TestNodeBuilder};

pub struct TestNetworkBuilder {
    pub num_nodes: u32,
}

impl TestNetworkBuilder {
    pub fn new() -> Self {
        Self { num_nodes: 3 }
    }

    pub fn with_num_nodes(mut self, num_nodes: u32) -> Self {
        self.num_nodes = num_nodes;
        self
    }

    /// Builds a new test network with the given number of nodes, and starts each of them.
    pub async fn build(self) -> Result<TestNetwork> {
        let temp_dir = tempdir()?;

        // Build and start the nodes.
        let mut nodes =
            join_all((0..self.num_nodes).map(|i| {
                TestNodeBuilder::new(temp_dir.path().join(format!("node-{}", i))).build()
            }))
            .await
            .into_iter()
            .collect::<Result<Vec<_>, _>>()?;

        // Wait for ready before building genesis.
        join_all(
            nodes
                .iter_mut()
                .map(|node| node.before_genesis_ready.wait()),
        )
        .await;

        // Build genesis.
        let genesis = {
            let mut builder = TestGenesisBuilder::default();
            for node in nodes.iter() {
                builder = builder.with_node(node);
            }
            builder.build()
        };

        // Apply genesis on each node.
        join_all(
            nodes
                .iter_mut()
                .map(|node| node.app.apply_genesis(genesis.clone())),
        )
        .await;

        // Wait for ready after genesis.
        join_all(nodes.iter_mut().map(|node| node.after_genesis_ready.wait())).await;

        let network = TestNetwork::new(temp_dir, nodes).await?;
        Ok(network)
    }
}
