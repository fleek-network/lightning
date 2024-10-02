use std::collections::HashMap;

use anyhow::Result;
use futures::future::join_all;
use lightning_interfaces::types::{Genesis, NodeIndex};
use tempfile::TempDir;

use super::{TestNetworkBuilder, TestNode};

/// A network of test nodes.
///
/// This encapsulates the management of nodes and provides methods to interact with them.
pub struct TestNetwork {
    _temp_dir: TempDir,
    pub genesis: Genesis,
    pub node_by_id: HashMap<NodeIndex, TestNode>,
}

impl TestNetwork {
    pub async fn new(temp_dir: TempDir, genesis: Genesis, nodes: Vec<TestNode>) -> Result<Self> {
        Ok(Self {
            _temp_dir: temp_dir,
            genesis,
            // We assume that at this point the genesis has been applied, otherwise this will panic.
            node_by_id: nodes
                .into_iter()
                .map(|node| (node.index(), node))
                .collect::<HashMap<_, _>>(),
        })
    }

    pub fn builder() -> TestNetworkBuilder {
        TestNetworkBuilder::new()
    }

    pub fn nodes(&self) -> impl Iterator<Item = &TestNode> {
        self.node_by_id.values()
    }

    pub fn maybe_node(&self, node_id: NodeIndex) -> Option<&TestNode> {
        self.node_by_id.get(&node_id)
    }

    pub fn node(&self, node_id: NodeIndex) -> &TestNode {
        self.node_by_id.get(&node_id).expect("node not found")
    }

    pub fn node_count(&self) -> usize {
        self.node_by_id.len()
    }

    pub async fn shutdown(&mut self) {
        join_all(self.node_by_id.values_mut().map(|node| node.shutdown())).await;
    }
}
