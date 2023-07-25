use fleek_crypto::NodeNetworkingPublicKey;

use crate::query::NodeInfo;

pub const MAX_BUCKET_SIZE: usize = 6;
pub const MAX_BUCKETS: usize = HASH_LEN * 8;
pub const HASH_LEN: usize = 32;

pub struct Bucket {
    inner: [NodeInfo; MAX_BUCKET_SIZE],
}

pub struct Table {
    local_node_key: NodeNetworkingPublicKey,
    buckets: Vec<Bucket>,
}

impl Table {
    pub fn closest_nodes(&self, target: &NodeNetworkingPublicKey) -> Vec<NodeInfo> {
        let index = bucket_index(&self.local_node_key, target);
        // Todo: Filter good vs bad nodes based on some criteria.
        self.buckets[index].inner.to_vec()
    }
}

fn bucket_index(key_a: &NodeNetworkingPublicKey, key_b: &NodeNetworkingPublicKey) -> usize {
    key_a
        .0
        .iter()
        .zip(key_b.0.iter())
        .map(|(a, b)| a ^ b)
        .position(|byte| byte == 1)
        .unwrap_or(0)
}
