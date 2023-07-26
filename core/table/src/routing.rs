use anyhow::Result;
use fleek_crypto::NodeNetworkingPublicKey;

use crate::query::NodeInfo;

pub const MAX_BUCKET_SIZE: usize = 6;
pub const MAX_BUCKETS: usize = HASH_LEN * 8;
pub const HASH_LEN: usize = 32;

#[derive(Clone)]
pub struct Node {
    info: NodeInfo,
}

pub struct Bucket {
    inner: Vec<Node>,
}

impl Bucket {
    fn new() -> Self {
        Bucket { inner: Vec::new() }
    }

    fn into_iter(self) -> impl Iterator<Item = Node> {
        self.inner.into_iter()
    }

    fn iter(&self) -> impl Iterator<Item = &Node> {
        self.inner.iter()
    }

    fn add_node(&mut self, node: &Node) -> bool {
        if self.inner.len() == MAX_BUCKET_SIZE {
            return false;
        }
        self.inner.push(node.clone());
        true
    }
}

pub struct Table {
    local_node_key: NodeNetworkingPublicKey,
    buckets: Vec<Bucket>,
}

impl Table {
    pub fn closest_nodes(&self, target: &NodeNetworkingPublicKey) -> Vec<NodeInfo> {
        let index = leading_zero_bits(&self.local_node_key, target);
        // Todo: Filter good vs bad nodes based on some criteria.
        // Todo: Return all our nodes from closest to furthest to target.
        self.buckets[index]
            .iter()
            .map(|node| node.info.clone())
            .collect()
    }

    fn add_node(&self, node: Node) -> Result<()> {
        if node.info.key == self.local_node_key {
            // We don't add ourselves to the routing table.
            return Ok(());
        }
        Ok(())
    }

    fn _add_node(&mut self, node: Node) {
        // Get index of bucket.
        let index = leading_zero_bits(&self.local_node_key, &node.info.key);
        assert_ne!(index, MAX_BUCKETS);
        let bucket_index = calculate_bucket_index(self.buckets.len(), index);
        if !self.buckets[bucket_index].add_node(&node) {
            if self.split_bucket(bucket_index) {
                self._add_node(node)
            }
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

        for node in bucket.into_iter() {
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

fn leading_zero_bits(key_a: &NodeNetworkingPublicKey, key_b: &NodeNetworkingPublicKey) -> usize {
    let distance = key_a
        .0
        .iter()
        .zip(key_b.0.iter())
        .map(|(a, b)| a ^ b)
        .collect::<Vec<_>>();
    let mut index = 0;
    for byte in distance {
        let leading_zeros = byte.leading_zeros();
        index += leading_zeros;
        if leading_zeros < 8 {
            break;
        }
    }
    index as usize
}
