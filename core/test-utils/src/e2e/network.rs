use std::collections::HashMap;

use anyhow::Result;
use futures::future::join_all;
use lightning_interfaces::types::NodeIndex;
use tempfile::TempDir;

use super::TestNode;

/// A network of test nodes.
///
/// This encapsulates the management of nodes and provides methods to interact with them.
pub struct TestNetwork {
    _temp_dir: TempDir,
    pub node_by_id: HashMap<NodeIndex, TestNode>,
}

impl TestNetwork {
    pub async fn new(temp_dir: TempDir, nodes: Vec<TestNode>) -> Result<Self> {
        Ok(Self {
            _temp_dir: temp_dir,

            // We assume that at this point the genesis has been applied, otherwise this will panic.
            node_by_id: nodes
                .into_iter()
                .map(|node| (node.get_id().expect("node id not found"), node))
                .collect::<HashMap<_, _>>(),
        })
    }

    pub fn nodes(&self) -> impl Iterator<Item = &TestNode> {
        self.node_by_id.values()
    }

    pub fn node(&self, node_id: NodeIndex) -> Option<&TestNode> {
        self.node_by_id.get(&node_id)
    }

    pub async fn shutdown(&mut self) {
        join_all(self.node_by_id.values_mut().map(|node| node.shutdown())).await;
    }
}
