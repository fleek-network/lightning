use std::collections::HashMap;

use anyhow::Result;
use futures::future::join_all;
use lightning_interfaces::types::{Genesis, NodeIndex};
use tempfile::TempDir;

use super::{BoxedTestNode, TestNetworkBuilder};

/// A network of test nodes.
///
/// This encapsulates the management of nodes and provides methods to interact with them.
pub struct TestNetwork {
    _temp_dir: TempDir,
    pub genesis: Genesis,
    pub node_by_id: HashMap<NodeIndex, Option<BoxedTestNode>>,
}

impl TestNetwork {
    pub async fn new(
        temp_dir: TempDir,
        genesis: Genesis,
        nodes: Vec<BoxedTestNode>,
    ) -> Result<Self> {
        Ok(Self {
            _temp_dir: temp_dir,
            genesis,
            // We assume that at this point the genesis has been applied, otherwise this will panic.
            node_by_id: nodes
                .into_iter()
                .map(|node| (node.index(), Some(node)))
                .collect::<HashMap<_, _>>(),
        })
    }

    pub fn builder() -> TestNetworkBuilder {
        TestNetworkBuilder::new()
    }

    pub fn nodes(&self) -> impl Iterator<Item = &BoxedTestNode> {
        self.node_by_id.values().map(|node| node.as_ref().unwrap())
    }

    pub fn maybe_node(&self, node_id: NodeIndex) -> Option<&BoxedTestNode> {
        self.node_by_id.get(&node_id).and_then(|node| node.as_ref())
    }

    pub fn node(&self, node_id: NodeIndex) -> &BoxedTestNode {
        self.maybe_node(node_id).expect("node not found")
    }

    pub fn node_count(&self) -> usize {
        self.node_by_id.len()
    }

    pub async fn shutdown(&mut self) {
        join_all(
            self.node_by_id
                .iter_mut()
                .filter_map(|(_, node_opt)| node_opt.take().map(|node| node.shutdown())),
        )
        .await;
    }
}
