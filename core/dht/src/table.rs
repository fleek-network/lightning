use std::collections::{BTreeMap, HashSet};

use anyhow::Result;
use fleek_crypto::NodeNetworkingPublicKey;
use lightning_interfaces::Blake3Hash;
use thiserror::Error;
use tokio::sync::{mpsc::Receiver, oneshot};

use crate::{
    bucket::{Bucket, MAX_BUCKETS, MAX_BUCKET_SIZE},
    distance,
    query::NodeInfo,
};

pub type TableKey = Blake3Hash;

pub async fn start_worker(mut rx: Receiver<TableRequest>, local_key: NodeNetworkingPublicKey) {
    let mut table = Table::new(local_key);
    while let Some(request) = rx.recv().await {
        match request {
            TableRequest::ClosestNodes { target: key, tx } => {
                let nodes = table.closest_nodes(&key);
                tx.send(Ok(nodes))
                    .expect("internal table client not to drop the channel");
            },
            TableRequest::AddNode { node, tx } => {
                let result = table.add_node(node).map_err(|e| QueryError(e.to_string()));
                if let Some(tx) = tx {
                    tx.send(result)
                        .expect("internal table client not to drop the channel");
                }
            },
            TableRequest::FirstNonEmptyBucket { tx } => {
                let local_key = table.local_node_key;
                let closest = table.closest_nodes(&local_key.0);
                match &closest.first() {
                    Some(node) => {
                        let index = distance::leading_zero_bits(&node.key.0, &local_key.0);
                        tx.send(Some(index))
                            .expect("internal table client not to drop the channel");
                    },
                    None => {
                        tx.send(None)
                            .expect("internal table client not to drop the channel");
                    },
                }
            },
        }
    }
}

#[derive(Debug, Error)]
#[error("querying the table failed: {0}")]
pub struct QueryError(String);

pub enum TableRequest {
    ClosestNodes {
        target: TableKey,
        tx: oneshot::Sender<Result<Vec<NodeInfo>, QueryError>>,
    },
    AddNode {
        node: NodeInfo,
        tx: Option<oneshot::Sender<Result<(), QueryError>>>,
    },
    // Returns index for non-empty bucket containing closest nodes. Used for bootstrapping.
    FirstNonEmptyBucket {
        tx: oneshot::Sender<Option<usize>>,
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
            buckets: vec![Bucket::new()],
        }
    }

    // Returns closest nodes to target in increasing order.
    pub fn closest_nodes(&self, target: &TableKey) -> Vec<NodeInfo> {
        // Todo: Filter good vs bad nodes based on some criteria.
        let mut closest = BTreeMap::new();
        let distance = distance::distance(&self.local_node_key.0, target);
        // We use this set to avoid traversing a bucket more than once since
        // some bits in the XORed value could be mapped to the same bucket.
        let mut visited = HashSet::new();
        let mut zero_bit_indexes = Vec::new();
        // First, visit every bucket, such that its corresponding bit in the XORed value is 1,
        // in decreasing order from MSB.
        for (count, byte) in distance.iter().enumerate() {
            let mask = 128u8;
            for shift in 0..8u8 {
                let possible_index = count * 8 + shift as usize;
                let index = calculate_bucket_index(self.buckets.len(), possible_index);
                if visited.contains(&index) {
                    continue;
                }
                if (byte & (mask >> shift)) > 0 {
                    for node in self.buckets[index].nodes() {
                        let distance = distance::distance(target, &node.key.0);
                        closest.insert(distance, node.clone());
                        if closest.len() >= MAX_BUCKET_SIZE {
                            return closest.into_values().collect();
                        }
                    }
                } else {
                    zero_bit_indexes.push(index)
                }
                visited.insert(index);
            }
        }

        // Second, visit every bucket, such that its corresponding bit in the XORed value is 0,
        // in increasing order from LSB.
        for index in zero_bit_indexes.iter().rev() {
            for node in self.buckets[*index].nodes() {
                let distance = distance::distance(target, &node.key.0);
                closest.insert(distance, node.clone());
                if closest.len() >= MAX_BUCKET_SIZE {
                    return closest.into_values().collect();
                }
            }
        }
        closest.into_values().collect()
    }

    fn add_node(&mut self, node: NodeInfo) -> Result<()> {
        if node.key == self.local_node_key {
            // We don't add ourselves to the routing table.
            return Ok(());
        }
        self._add_node(node);
        Ok(())
    }

    fn _add_node(&mut self, node: NodeInfo) {
        // Get index of bucket.
        let index = distance::leading_zero_bits(&self.local_node_key.0, &node.key.0);
        assert_ne!(index, MAX_BUCKETS);
        let bucket_index = calculate_bucket_index(self.buckets.len(), index);
        if !self.buckets[bucket_index].add_node(node.clone()) && self.split_bucket(bucket_index) {
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
