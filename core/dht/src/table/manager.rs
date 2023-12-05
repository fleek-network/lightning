use std::collections::{BTreeMap, HashSet};

use anyhow::Result;
use fleek_crypto::NodePublicKey;
use lightning_interfaces::types::NodeIndex;

use crate::table::bucket::{Bucket, MAX_BUCKETS, MAX_BUCKET_SIZE};
use crate::table::worker::TableKey;
use crate::table::{distance, NodeInfo};

pub enum Event {
    Discovery { from: NodeIndex },
    Pong { from: NodeIndex, timestamp: u64 },
    Unresponsive { index: NodeIndex },
}

pub trait Manager: Send + 'static {
    fn closest_contacts(&self, key: TableKey) -> Vec<NodeIndex>;

    fn closest_contact_info(&self, key: TableKey) -> Vec<NodeInfo>;

    fn add_node(&mut self, node: NodeInfo) -> Result<()>;

    fn handle_event(&mut self, event: Event);
}

impl Manager for TableManager {
    fn closest_contacts(&self, key: TableKey) -> Vec<NodeIndex> {
        self.closest_nodes(&key)
            .into_iter()
            .map(|info| info.index)
            .collect()
    }

    fn closest_contact_info(&self, key: TableKey) -> Vec<NodeInfo> {
        self.closest_nodes(&key)
    }

    fn add_node(&mut self, node: NodeInfo) -> Result<()> {
        self.inner_add_node(node)
    }

    fn handle_event(&mut self, _event: Event) {
        todo!()
    }
}

pub struct TableManager {
    pub local_node_key: NodePublicKey,
    pub buckets: Vec<Bucket>,
}

impl TableManager {
    pub fn new(local_node_key: NodePublicKey) -> Self {
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

    fn _get_bucket_nodes(&self, bucket_index: usize) -> Option<Vec<NodeInfo>> {
        if bucket_index < self.buckets.len() {
            Some(self.buckets[bucket_index].nodes().cloned().collect())
        } else {
            None
        }
    }

    fn _set_bucket_nodes(&mut self, bucket_index: usize, nodes: Vec<NodeInfo>) {
        if bucket_index < self.buckets.len() {
            self.buckets[bucket_index].set_nodes(nodes);
        }
    }

    fn inner_add_node(&mut self, node: NodeInfo) -> Result<()> {
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

    pub fn _update_node_timestamp(&mut self, node_key: NodePublicKey, timestamp: u64) {
        let index = distance::leading_zero_bits(&self.local_node_key.0, &node_key.0);
        let bucket_index = calculate_bucket_index(self.buckets.len(), index);
        for node in self.buckets[bucket_index].nodes_mut() {
            if node.key == node_key {
                node.last_responded = Some(timestamp);
            }
        }
    }

    fn split_bucket(&mut self, index: usize) -> bool {
        // We split the bucket only if it is the last one.
        // This is because the closest nodes to us will be
        // in the buckets further up in the list.
        // In addition, bucket size decrements.
        if index == MAX_BUCKETS - 1 || index != self.buckets.len() - 1 {
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

// Todo: refactor and bring back tests.
