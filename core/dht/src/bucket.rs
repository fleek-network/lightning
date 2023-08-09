use crate::query::NodeInfo;

pub const MAX_BUCKET_SIZE: usize = 6;
pub const MAX_BUCKETS: usize = HASH_LEN * 8;
pub const HASH_LEN: usize = 32;

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

    // Todo: Handle duplicates.
    pub fn add_node(&mut self, node: NodeInfo) -> bool {
        if self.inner.len() == MAX_BUCKET_SIZE {
            return false;
        }

        if let Some(index) = self.inner.iter().position(|member| member.key == node.key) {
            self.inner.insert(index, node);
            return true;
        }

        self.inner.push(node);
        true
    }
}
