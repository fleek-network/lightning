use std::collections::BTreeMap;

use anyhow::Result;
use fleek_crypto::NodeNetworkingPublicKey;
use lightning_interfaces::Blake3Hash;
use thiserror::Error;
use tokio::sync::{mpsc::Receiver, oneshot};

use crate::{
    bucket::{Bucket, Node, MAX_BUCKETS, MAX_BUCKET_SIZE},
    distance,
    query::NodeInfo,
};

pub type TableKey = Blake3Hash;

#[derive(Debug, Error)]
#[error("querying the table failed: {0}")]
pub struct QueryError(String);

pub enum TableQuery {
    ClosestNodes {
        target: TableKey,
        tx: oneshot::Sender<Result<Vec<NodeInfo>, QueryError>>,
    },
    AddNode {
        node: Node,
        tx: oneshot::Sender<Result<(), QueryError>>,
    },
}

pub struct Table {
    local_node_key: NodeNetworkingPublicKey,
    buckets: Vec<Bucket>,
}

impl Table {
    pub fn new(local_node_key: NodeNetworkingPublicKey) -> Self {
        Self {
            local_node_key,
            buckets: Vec::new(),
        }
    }

    pub fn closest_nodes(&self, target: &TableKey) -> Vec<NodeInfo> {
        // Todo: Filter good vs bad nodes based on some criteria.
        let mut closest = BTreeMap::new();
        let distance = distance::distance(&self.local_node_key.0, target);
        let mut zero_bit_indexes = Vec::new();
        // First, visit every bucket, such that its corresponding bit in the XORed value is 1,
        // in decreasing order from MSB.
        for (count, byte) in distance.iter().enumerate() {
            let mask = 128u8;
            for shift in 0..8u8 {
                let index = count * 8 + shift as usize;
                if (byte & (mask >> shift)) > 0 {
                    for node in self.buckets[index].nodes() {
                        let distance = distance::distance(target, &node.info.key.0);
                        closest.insert(distance, node.info.clone());
                        if closest.len() >= MAX_BUCKET_SIZE {
                            return closest.into_values().collect();
                        }
                    }
                } else {
                    zero_bit_indexes.push(index)
                }
            }
        }

        // Second, visit every bucket, such that its corresponding bit in the XORed value is 0,
        // in increasing order from LSB.
        for index in zero_bit_indexes.iter().rev() {
            for node in self.buckets[*index].nodes() {
                let distance = distance::distance(target, &node.info.key.0);
                closest.insert(distance, node.info.clone());
                if closest.len() >= MAX_BUCKET_SIZE {
                    return closest.into_values().collect();
                }
            }
        }
        closest.into_values().collect()
    }

    fn add_node(&mut self, node: Node) -> Result<()> {
        if node.info.key == self.local_node_key {
            // We don't add ourselves to the routing table.
            return Ok(());
        }
        self._add_node(node);
        Ok(())
    }

    fn _add_node(&mut self, node: Node) {
        // Get index of bucket.
        let index = distance::leading_zero_bits(&self.local_node_key.0, &node.info.key.0);
        assert_ne!(index, MAX_BUCKETS);
        let bucket_index = calculate_bucket_index(self.buckets.len(), index);
        if !self.buckets[bucket_index].add_node(&node) && self.split_bucket(bucket_index) {
            self._add_node(node)
        }
    }

    fn split_bucket(&mut self, index: usize) -> bool {
        // We split the bucket only if it is the last one.
        // This is because the closest nodes to us will be
        // in the buckets further up in the list.
        // In addition, bucket size decrements.
        if index != MAX_BUCKETS - 1 || index != self.buckets.len() - 1 {
            return false;
        }

        let bucket = self.buckets.pop().expect("there to be at least one bucket");
        self.buckets.push(Bucket::new());
        self.buckets.push(Bucket::new());

        for node in bucket.into_nodes() {
            self._add_node(node)
        }
        true
    }
}

fn calculate_bucket_index(bucket_count: usize, possible_index: usize) -> usize {
    if possible_index >= bucket_count {
        bucket_count - 1
    } else {
        possible_index
    }
}

pub async fn start_server(mut rx: Receiver<TableQuery>, local_key: NodeNetworkingPublicKey) {
    let mut table = Table::new(local_key);
    while let Some(query) = rx.recv().await {
        match query {
            TableQuery::ClosestNodes { target: key, tx } => {
                let nodes = table.closest_nodes(&key);
                if tx.send(Ok(nodes)).is_err() {
                    tracing::error!("failed to send Table query response")
                }
            },
            TableQuery::AddNode { node, tx } => {
                let nodes = table.add_node(node).map_err(|e| QueryError(e.to_string()));
                if tx.send(nodes).is_err() {
                    tracing::error!("failed to send Table query response")
                }
            },
        }
    }
}
