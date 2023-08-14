use std::{
    collections::{hash_map::Entry, HashMap},
    net::SocketAddr,
    sync::Arc,
};

use anyhow::Result;
use fleek_crypto::NodeNetworkingPublicKey;
use lightning_interfaces::types::{KeyPrefix, TableEntry};
use tokio::{
    net::UdpSocket,
    select,
    sync::{
        mpsc,
        mpsc::{Receiver, Sender},
        oneshot, Notify,
    },
};

use crate::{
    query::{Message, MessageType, NodeInfo, Query, Response},
    socket,
    store::StoreRequest,
    table::{TableKey, TableRequest},
    task::ResponseEvent,
};

pub const NO_REPLY_CHANNEL_ID: u64 = 0;

pub async fn start_worker(
    response_queue_tx: Sender<ResponseEvent>,
    mut request_rx: Receiver<HandlerRequest>,
    table_tx: Sender<TableRequest>,
    store_tx: Sender<StoreRequest>,
    socket: Arc<UdpSocket>,
    local_key: NodeNetworkingPublicKey,
    shutdown_notify: Arc<Notify>,
) {
    let mut handler = Handler {
        response_queue_tx,
        local_key,
        table_tx: table_tx.clone(),
        store_tx,
        socket: socket.clone(),
        received_shutdown: false,
    };
    loop {
        if handler.received_shutdown {
            tracing::trace!("shutting down handler worker");
            break;
        }
        select! {
            incoming = socket::recv_from(&socket) => {
                match incoming {
                    Ok((datagram, address)) => {
                        if let Err(e) = handler.handle_incoming(datagram, address) {
                            tracing::error!("failed to handle incoming datagram: {e:?}");
                        }
                    }
                    Err(e) => {
                        tracing::error!("unexpected error when reading from socket: {e:?}")
                    }
                }
            }
            _ = shutdown_notify.notified() => {
                tracing::info!("shutting down handler");
                break;
            }
        }
    }
}

async fn handle_query(
    table_tx: Sender<TableRequest>,
    store_tx: Sender<StoreRequest>,
    socket: Arc<UdpSocket>,
    local_key: NodeNetworkingPublicKey,
    message: Message,
    address: SocketAddr,
) -> Result<()> {
    let query: Query = bincode::deserialize(&message.payload)?;
    match query {
        Query::Find { find_value, target } => {
            let (tx, rx) = oneshot::channel();
            table_tx
                .send(TableRequest::ClosestNodes { target, tx })
                .await
                .expect("table worker to not drop the channel");
            let nodes = rx.await.expect("table worker to not drop the channel")?;
            let value = match find_value {
                true => {
                    let (get_tx, get_rx) = oneshot::channel();
                    store_tx
                        .send(StoreRequest::Get {
                            key: target,
                            tx: get_tx,
                        })
                        .await
                        .expect("store worker not to drop channel");
                    get_rx.await.expect("store worker not to drop channel")
                },
                false => None,
            };
            let payload = bincode::serialize(&Response { nodes, value })?;
            let response = Message {
                ty: MessageType::Response,
                token: message.token,
                id: message.id,
                sender_key: local_key,
                payload,
            };
            let bytes = bincode::serialize(&response)?;
            socket::send_to(&socket, bytes.as_slice(), address).await?;

            tracing::trace!(
                "discovered new contact {:?} {:?}",
                message.sender_key,
                address
            );
            table_tx
                .send(TableRequest::AddNode {
                    node: NodeInfo {
                        address,
                        key: message.sender_key,
                    },
                    tx: None,
                })
                .await
                .expect("table worker to not drop the channel");
        },
        Query::Store { key, value } => {
            store_tx
                .send(StoreRequest::Put { key, value })
                .await
                .expect("store worker not to drop channel");
        },
        Query::Ping => {
            let payload = bincode::serialize(&Response {
                nodes: Vec::new(),
                value: None,
            })?;
            let response = Message {
                ty: MessageType::Response,
                id: message.id,
                token: message.token,
                sender_key: local_key,
                payload,
            };
            let bytes = bincode::serialize(&response)?;
            socket::send_to(&socket, bytes.as_slice(), address).await?;
        },
    }
    Ok(())
}

#[derive(Debug)]
pub enum HandlerRequest {
    Get {
        key: Vec<u8>,
        tx: oneshot::Sender<Result<Option<TableEntry>>>,
    },
    Put {
        key: Vec<u8>,
        value: Vec<u8>,
    },
    FindNode {
        target: NodeNetworkingPublicKey,
        tx: oneshot::Sender<Result<Vec<NodeInfo>>>,
    },
    Shutdown,
}

struct Handler {
    response_queue_tx: Sender<ResponseEvent>,
    local_key: NodeNetworkingPublicKey,
    table_tx: Sender<TableRequest>,
    store_tx: Sender<StoreRequest>,
    socket: Arc<UdpSocket>,
    received_shutdown: bool,
}

impl Handler {
    fn handle_incoming(&mut self, datagram: Vec<u8>, address: SocketAddr) -> Result<()> {
        let message: Message = bincode::deserialize(datagram.as_slice())?;
        match message.ty {
            MessageType::Query => {
                tokio::spawn(handle_query(
                    self.table_tx.clone(),
                    self.store_tx.clone(),
                    self.socket.clone(),
                    self.local_key,
                    message,
                    address,
                ));
            },
            MessageType::Response => {
                // This should provide some protection against unrequested replies.
                let response: Response = bincode::deserialize(&message.payload)?;
                let event_tx = self.response_queue_tx.clone();
                tokio::spawn(async move {
                    if event_tx
                        .send(ResponseEvent {
                            id: message.token,
                            sender_key: message.sender_key,
                            response,
                        })
                        .await
                        .is_err()
                    {
                        tracing::error!("failed to send response to lookup task")
                    }
                });
            },
        }
        Ok(())
    }
}
