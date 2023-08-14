use std::sync::Arc;

use anyhow::Result;
use fleek_crypto::NodeNetworkingPublicKey;
use lightning_interfaces::types::{KeyPrefix, TableEntry};
use tokio::{
    net::UdpSocket,
    select,
    sync::{
        mpsc::{Receiver, Sender},
        oneshot, Notify,
    },
};

use crate::{
    handler::NO_REPLY_CHANNEL_ID,
    query::{Message, MessageType, Query},
    socket,
    table::TableKey,
    task::Task,
};

pub enum DhtRequest {
    Get {
        key: Vec<u8>,
        tx: oneshot::Sender<Result<Option<TableEntry>>>,
    },
    Put {
        key: Vec<u8>,
        value: Vec<u8>,
    },
}

pub async fn start_worker(
    mut rx: Receiver<DhtRequest>,
    task_tx: Sender<Task>,
    bootstrap_notify: Arc<Notify>,
    shutdown_notify: Arc<Notify>,
    local_key: NodeNetworkingPublicKey,
    socket: Arc<UdpSocket>,
) {
    let handler = ApiHandler {
        task_tx,
        local_key,
        socket,
    };
    loop {
        select! {
            request = rx.recv() => {
                let request = request.unwrap();
                match request {
                    DhtRequest::Get { key, tx } => {
                        let get_handler = handler.clone();
                        tokio::spawn(async move {
                            if let Err(e) = get_handler.handle_get(key, tx).await {
                                tracing::error!("failed to handle GET");
                            }
                        });
                    }
                    DhtRequest::Put { key, value } => {
                        let put_handler = handler.clone();
                        tokio::spawn(async move {
                            if let Err(e) = put_handler.handle_put(key, value).await {
                                tracing::error!("failed to handle GET");
                            }
                        });
                    }
                }
            }
            _ = bootstrap_notify.notified() => {
                let bootstrap_handler = handler.clone();
                tokio::spawn(async move {
                    if let Err(e) = bootstrap_handler.bootstrap().await {
                        tracing::error!("failed to start bootstrap task");
                    }
                });
            }
            _ = shutdown_notify.notified() => {
                break;
            }
        }
    }
}

#[derive(Clone)]
pub struct ApiHandler {
    task_tx: Sender<Task>,
    local_key: NodeNetworkingPublicKey,
    socket: Arc<UdpSocket>,
}

impl ApiHandler {
    async fn handle_get(
        &self,
        key: Vec<u8>,
        response_tx: oneshot::Sender<Result<Option<TableEntry>>>,
    ) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        let target = TableKey::try_from(key.as_slice())?;
        let task = Task::Lookup { target, tx };
        self.task_tx.send(task).await?;
        let response = rx.await?;

        if response.value.is_none() {
            if response_tx.send(Ok(None)).is_err() {
                tracing::error!("client dropped the channel")
            }
        } else {
            let entry = TableEntry {
                prefix: KeyPrefix::ContentRegistry,
                key,
                value: response.value.unwrap_or_default(),
                // Todo: Refactor when source is implemented.
                source: response
                    .source
                    .unwrap_or(NodeNetworkingPublicKey(rand::random())),
                signature: None,
            };
            if response_tx.send(Ok(Some(entry))).is_err() {
                tracing::error!("client dropped the channel")
            }
        }
        Ok(())
    }

    async fn handle_put(&self, key: Vec<u8>, value: Vec<u8>) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        let target = TableKey::try_from(key.as_slice())?;
        let task = Task::Lookup { target, tx };
        self.task_tx.send(task).await?;
        let response = rx.await?;

        let payload =
            bincode::serialize(&Query::Store { key: target, value }).expect("query to be valid");
        let message = Message {
            ty: MessageType::Query,
            id: NO_REPLY_CHANNEL_ID,
            token: rand::random(),
            sender_key: self.local_key,
            payload,
        };
        let bytes = bincode::serialize(&message).expect("Serialization to succeed");
        for node in response.nodes {
            tracing::trace!("send STORE to {node:?}");
            if let Err(e) = socket::send_to(&self.socket, &bytes, node.address).await {
                tracing::error!("failed to send datagram {e:?}");
            }
        }
        Ok(())
    }

    async fn bootstrap(&self) -> Result<()> {
        self.task_tx.send(Task::Bootstrap).await?;
        Ok(())
    }
}
