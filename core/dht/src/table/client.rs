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
    pub async fn get(&self, key: TableKey) -> Option<Bytes> {
        let (respond_tx, respond_rx) = oneshot::channel();
        if self
            .request_queue
            .send(Request::Get {
                key: rand::random(),
                local: false,
                respond: respond_tx,
            })
            .await
            .is_err()
        {
            return None;
        }
        match respond_rx.await.ok()? {
            Ok(value) => value,
            Err(e) => {
                tracing::debug!("failed to find entry for key {key:?}: {e:?}");
                None
            },
        }
    }

    pub async fn put(&self, key: TableKey, value: Bytes) {
        let _ = self
            .request_queue
            .send(Request::Put {
                key,
                value,
                local: false,
            })
            .await;
    }

    pub async fn local_get(&self, key: TableKey) -> Option<Bytes> {
        let (respond_tx, respond_rx) = oneshot::channel();
        if self
            .request_queue
            .send(Request::Get {
                key: rand::random(),
                local: true,
                respond: respond_tx,
            })
            .await
            .is_err()
        {
            return None;
        }
        match respond_rx.await.ok()? {
            Ok(value) => value,
            Err(e) => {
                tracing::debug!("failed to find entry for key {key:?}: {e:?}");
                None
            },
        }
    }

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

    pub fn bootstrap(&self, bootstrappers: Vec<NodeIndex>) -> Result<()> {
        self.request_queue
            .try_send(Request::Bootstrap {
                bootstrap_nodes: bootstrappers,
            })
            .map_err(Into::into)
    }
}
