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
    lookup,
    lookup::{LookupResult, LookupTask, ResponseEvent},
    query::{Message, MessageType, NodeInfo, Query, Response},
    socket,
    store::StoreRequest,
    table::{TableKey, TableRequest},
};

pub const NO_REPLY_CHANNEL_ID: u64 = 0;

pub async fn start_worker(
    mut request_rx: Receiver<HandlerRequest>,
    table_tx: Sender<TableRequest>,
    store_tx: Sender<StoreRequest>,
    socket: Arc<UdpSocket>,
    local_key: NodeNetworkingPublicKey,
    shutdown_notify: Arc<Notify>,
) {
    let mut handler = Handler {
        pending: HashMap::new(),
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
            request = request_rx.recv() => {
                if request.is_none() {
                    break;
                }
                if let Err(e) = handler.handle_request(request.unwrap()) {
                    tracing::error!("failed to handle request: {e:?}");
                }
            }
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
    pending: HashMap<u64, Sender<ResponseEvent>>,
    local_key: NodeNetworkingPublicKey,
    table_tx: Sender<TableRequest>,
    store_tx: Sender<StoreRequest>,
    socket: Arc<UdpSocket>,
    received_shutdown: bool,
}

impl Handler {
    fn handle_request(&mut self, request: HandlerRequest) -> Result<()> {
        let (event_tx, event_rx) = mpsc::channel(100);
        let task_id = rand::random();
        self.pending.insert(task_id, event_tx);

        match request {
            HandlerRequest::Get { key, tx } => {
                let target = TableKey::try_from(key.as_slice())?;
                let task = LookupTask::new(
                    task_id,
                    true,
                    self.local_key,
                    target,
                    self.table_tx.clone(),
                    event_rx,
                    self.socket.clone(),
                );

                tokio::spawn(async move {
                    // Todo: refactor lookup to not return an enum.
                    match lookup::lookup(task).await {
                        Ok(lookup_result) => {
                            let value = match lookup_result {
                                LookupResult::Nodes(_) => panic!("we did not request for a nodes"),
                                LookupResult::Value(value) => value,
                            };

                            let entry = TableEntry {
                                prefix: KeyPrefix::ContentRegistry,
                                key,
                                value: value.unwrap_or_default(),
                                // Todo: make sure we keep track of source.
                                source: NodeNetworkingPublicKey(rand::random()),
                                signature: None,
                            };

                            if tx.send(Ok(Some(entry))).is_err() {
                                tracing::error!("client dropped channel for Get respose")
                            }
                        },
                        Err(e) => {
                            tracing::error!("lookup failed: {e:?}");
                            if tx.send(Err(e.into())).is_err() {
                                tracing::error!("client dropped channel for Get respose")
                            }
                        },
                    }
                });
            },
            HandlerRequest::Put { key, value } => {
                let socket_clone = self.socket.clone();
                let sender_key = self.local_key;
                let target = TableKey::try_from(key.as_slice())?;
                let task = LookupTask::new(
                    task_id,
                    false,
                    self.local_key,
                    target,
                    self.table_tx.clone(),
                    event_rx,
                    self.socket.clone(),
                );

                tokio::spawn(async move {
                    let nodes = match lookup::lookup(task).await {
                        Ok(lookup_result) => match lookup_result {
                            LookupResult::Nodes(nodes) => nodes,
                            LookupResult::Value(_) => panic!("we did not request for a nodes"),
                        },
                        Err(e) => {
                            tracing::error!("failed to handle PUT request: {e:?}");
                            return;
                        },
                    };

                    let payload = bincode::serialize(&Query::Store { key: target, value })
                        .expect("query to be valid");
                    let message = Message {
                        ty: MessageType::Query,
                        id: NO_REPLY_CHANNEL_ID,
                        token: rand::random(),
                        sender_key,
                        payload,
                    };
                    let bytes = bincode::serialize(&message).expect("Serialization to succeed");
                    for node in nodes {
                        tracing::trace!("send STORE to {node:?}");
                        if let Err(e) = socket::send_to(&socket_clone, &bytes, node.address).await {
                            tracing::error!("failed to send datagram {e:?}");
                        }
                    }
                });
            },
            HandlerRequest::FindNode { target, tx } => {
                let task = LookupTask::new(
                    task_id,
                    false,
                    self.local_key,
                    target.0,
                    self.table_tx.clone(),
                    event_rx,
                    self.socket.clone(),
                );

                tokio::spawn(async move {
                    match lookup::lookup(task).await {
                        Ok(lookup_result) => {
                            let nodes = match lookup_result {
                                LookupResult::Nodes(nodes) => nodes,
                                LookupResult::Value(_) => panic!("we did not request for a nodes"),
                            };
                            tx.send(Ok(nodes))
                                .expect("client dropped channel for FindNode respose")
                        },
                        Err(e) => {
                            tracing::error!("lookup failed: {e:?}");
                            tx.send(Err(e.into()))
                                .expect("client dropped channel for FindNode respose");
                        },
                    }
                });
            },
            HandlerRequest::Shutdown => self.received_shutdown = true,
        }
        Ok(())
    }

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
                if let Entry::Occupied(event_tx) = self.pending.entry(message.id) {
                    if event_tx.get().is_closed() {
                        event_tx.remove();
                    } else {
                        let response: Response = bincode::deserialize(&message.payload)?;
                        let event_tx = event_tx.get().clone();
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
                    }
                }
            },
        }
        Ok(())
    }
}
