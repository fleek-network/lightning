use std::time::Duration;

use crate::table::server::TableKey;
use crate::table::NodeInfo;

#[cfg(not(test))]
pub const MAX_BUCKET_SIZE: usize = 6;
#[cfg(test)]
pub const MAX_BUCKET_SIZE: usize = 3;
pub const MAX_BUCKETS: usize = HASH_LEN * 8;
pub const HASH_LEN: usize = 32;
pub const BUCKET_REFRESH_INTERVAL: Duration = Duration::from_secs(900); // 15 minutes

#[derive(Default)]
pub struct Bucket {
    inner: Vec<NodeInfo>,
}

impl Bucket {
    pub fn new() -> Self {
        Bucket::default()
    }

    pub fn into_nodes(self) -> impl Iterator<Item = NodeInfo> {
        self.inner.into_iter()
    }

    pub fn nodes(&self) -> impl Iterator<Item = &NodeInfo> {
        self.inner.iter()
    }

    pub fn nodes_mut(&mut self) -> impl Iterator<Item = &mut NodeInfo> {
        self.inner.iter_mut()
    }

    pub fn set_nodes(&mut self, nodes: Vec<NodeInfo>) {
        self.inner = nodes;
    }

    // Todo: Handle duplicates.
    pub fn add_node(&mut self, node: NodeInfo) -> bool {
        if self.inner.len() == MAX_BUCKET_SIZE {
            return false;
        }

        if let Some(index) = self.inner.iter().position(|member| member.key == node.key) {
            self.inner[index] = node;
            return true;
        }

        self.inner.push(node);
        true
    }
}

pub fn random_key_in_bucket(mut index: usize, local_key: &TableKey) -> TableKey {
    let mut key: TableKey = rand::random();
    for (byte, local_key_byte) in key.iter_mut().zip(local_key.iter()) {
        if index > 7 {
            *byte = *local_key_byte;
        } else {
            // The first index bits of the random key byte
            // have to match the byte of the local_key.
            let mask = 255_u8 >> index;
            *byte = (*byte & mask) | (*local_key_byte & !mask);
            // The index + 1 bit (from the left) of the random key byte
            // has to be different than the local key bit at that position.
            let mask = 128_u8 >> index;
            *byte = (*byte & !mask) | ((*local_key_byte ^ 255_u8) & mask);
            break;
        }
        index -= 8;
    }
    key
}
