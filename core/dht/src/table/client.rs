use anyhow::Result;
use bytes::Bytes;
use lightning_interfaces::types::NodeIndex;
use tokio::sync::mpsc::Sender;
use tokio::sync::oneshot;

use crate::table::server::{Request, TableKey};

#[derive(Clone)]
pub struct Client {
    request_queue: Sender<Request>,
}

impl Client {
    pub async fn get(&self, hash: u32) -> Option<Bytes> {
        None
    }

    pub async fn put(&self, hash: u32, value: Bytes) {}

    pub async fn local_get(&self, key: TableKey) -> Option<Bytes> {
        None
    }

    pub async fn local_put(&self, value: Bytes) {}

    pub fn try_local_put(&self, value: Bytes) -> Result<()> {
        self.request_queue
            .try_send(Request::Put {
                key: rand::random(),
                value,
                local: true,
            })
            .map_err(Into::into)
    }

    pub async fn closest_contacts(&self, key: TableKey) -> Result<Vec<NodeIndex>> {
        let (respond_tx, respond_rx) = oneshot::channel();
        self.request_queue
            .send(Request::ClosestContacts {
                key,
                respond: respond_tx,
            })
            .await?;
        respond_rx.await.map_err(Into::into)
    }

    pub fn bootstrap(&self, _bootstrappers: Vec<NodeIndex>) {}
}
