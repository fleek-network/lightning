use bytes::Bytes;
use lightning_interfaces::types::NodeIndex;
use tokio::sync::mpsc::Sender;

use crate::table::server::Request;

#[derive(Clone)]
pub struct Client {
    request_queue: Sender<Request>,
}

impl Client {
    pub async fn get(&self, hash: u32) -> Option<Bytes> {
        None
    }

    pub async fn put(&self, hash: u32, value: Bytes) {}

    pub async fn local_get(&self, hash: u32) -> Option<Bytes> {
        None
    }

    pub async fn local_put(&self, hash: u32, value: Bytes) {}

    pub async fn closest_contacts(&self, hash: u32) -> Vec<NodeIndex> {
        Vec::new()
    }

    pub fn bootstrap(&self, _bootstrappers: Vec<NodeIndex>) {}
}
