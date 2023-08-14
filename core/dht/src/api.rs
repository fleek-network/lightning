use std::sync::Arc;

use anyhow::Result;
use fleek_crypto::NodeNetworkingPublicKey;
use lightning_interfaces::types::{DhtRequest, DhtResponse, KeyPrefix, TableEntry};
use tokio::{
    net::UdpSocket,
    select,
    sync::{
        mpsc::{Receiver, Sender},
        oneshot, Notify,
    },
};

use crate::{
    query::{Message, MessageType, Query},
    socket,
    table::TableKey,
    task::Task,
};

pub const NO_REPLY_CHANNEL_ID: u64 = 0;

pub async fn start_worker(
    mut rx: Receiver<affair::Task<DhtRequest, DhtResponse>>,
    task_tx: Sender<Task>,
    bootstrap_notify: Arc<Notify>,
    shutdown_notify: Arc<Notify>,
    local_key: NodeNetworkingPublicKey,
    socket: Arc<UdpSocket>,
) {
    let handler = Handler {
        task_tx,
        local_key,
        socket,
    };
    loop {
        select! {
            task = rx.recv() => {
                let task = task.unwrap();
                match task.request.clone() {
                    DhtRequest::Get { key, .. } => {
                        let get_handler = handler.clone();
                        tokio::spawn(async move {
                            match get_handler.handle_get(key).await {
                                Ok(value) => task.respond(DhtResponse::Get(value)),
                                Err(e) => tracing::error!("failed to handle GET: {e:?}"),
                            }
                        });
                    }
                    DhtRequest::Put { key, value, .. } => {
                        let put_handler = handler.clone();
                        tokio::spawn(async move {
                            match put_handler.handle_put(key, value).await {
                                Ok(_) => task.respond(DhtResponse::Put(())),
                                Err(e) => tracing::error!("failed to handle PUT: {e:?}"),
                            }
                        });
                    }
                }
            }
            _ = bootstrap_notify.notified() => {
                let bootstrap_handler = handler.clone();
                tokio::spawn(async move {bootstrap_handler.bootstrap().await;});
            }
            _ = shutdown_notify.notified() => {
                break;
            }
        }
    }
}

#[derive(Clone)]
pub struct Handler {
    task_tx: Sender<Task>,
    local_key: NodeNetworkingPublicKey,
    socket: Arc<UdpSocket>,
}

impl Handler {
    async fn handle_get(&self, key: Vec<u8>) -> Result<Option<TableEntry>> {
        let (tx, rx) = oneshot::channel();
        let target = TableKey::try_from(key.as_slice())?;
        let task = Task::Lookup { target, tx };
        self.task_tx.send(task).await?;
        let response = rx.await?;

        if response.value.is_none() {
            Ok(None)
        } else {
            Ok(Some(TableEntry {
                prefix: KeyPrefix::ContentRegistry,
                key,
                value: response.value.unwrap_or_default(),
                // Todo: Refactor when source is implemented.
                source: response
                    .source
                    .unwrap_or(NodeNetworkingPublicKey(rand::random())),
                signature: None,
            }))
        }
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

    async fn bootstrap(&self) {
        if self.task_tx.send(Task::Bootstrap).await.is_err() {
            tracing::error!("failed to send bootstrap task");
        }
    }
}
