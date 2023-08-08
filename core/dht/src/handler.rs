use std::{
    collections::{hash_map::Entry, HashMap},
    net::SocketAddr,
    sync::Arc,
};

use anyhow::Result;
use fleek_crypto::NodeNetworkingPublicKey;
use lightning_interfaces::dht::{KeyPrefix, TableEntry};
use tokio::{
    net::UdpSocket,
    select,
    sync::{
        mpsc,
        mpsc::{Receiver, Sender},
        oneshot,
    },
};

use crate::{
    lookup,
    lookup::{LookupResult, LookupTask, ResponseEvent},
    query::{Message, MessageType, NodeInfo, Query, Response},
    socket,
    table::{TableCommand, TableKey},
};

pub const NO_REPLY_CHANNEL_ID: u64 = 0;

pub async fn start_worker(
    mut command_rx: Receiver<HandlerCommand>,
    table_tx: Sender<TableCommand>,
    socket: Arc<UdpSocket>,
    local_key: NodeNetworkingPublicKey,
) {
    let mut handler = Handler {
        pending: HashMap::new(),
        local_key,
        table_tx: table_tx.clone(),
        socket: socket.clone(),
        received_shutdown: false,
    };
    loop {
        if handler.received_shutdown {
            tracing::trace!("shutting down handler worker");
            break;
        }
        select! {
            command = command_rx.recv() => {
                if command.is_none() {
                    break;
                }
                if let Err(e) = handler.handle_command(command.unwrap()) {
                    tracing::error!("failed to handle incoming datagram: {e:?}");
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
        }
    }
}

async fn handle_query(
    table_tx: Sender<TableCommand>,
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
                .send(TableCommand::ClosestNodes { target, tx })
                .await
                .expect("table worker to not drop the channel");
            let nodes = rx.await.expect("table worker to not drop the channel")?;
            let value = match find_value {
                // Todo: Check from local store.
                true => todo!(),
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
        },
        Query::Store { .. } => {
            // Todo: How do we avoid someone sending tons of Store queries.
            todo!()
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
pub enum HandlerCommand {
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
    table_tx: Sender<TableCommand>,
    socket: Arc<UdpSocket>,
    received_shutdown: bool,
}

impl Handler {
    fn handle_command(&mut self, command: HandlerCommand) -> Result<()> {
        let (event_tx, event_rx) = mpsc::channel(100);
        let task_id = rand::random();
        self.pending.insert(task_id, event_tx);

        match command {
            HandlerCommand::Get { key, tx } => {
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
            HandlerCommand::Put { key, value } => {
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
                            tracing::error!("failed to handle PUT command: {e:?}");
                            return;
                        },
                    };

                    let payload = bincode::serialize(&Query::Store { key: target, value })
                        .expect("query to be valid");
                    // Todo: Add sender information in message.
                    let message = Message {
                        ty: MessageType::Query,
                        token: rand::random(),
                        sender_key,
                        id: NO_REPLY_CHANNEL_ID,
                        payload,
                    };
                    let bytes = bincode::serialize(&message).expect("Serialization to succeed");
                    for node in nodes {
                        if let Err(e) = socket::send_to(&socket_clone, &bytes, node.address).await {
                            tracing::error!("failed to send datagram {e:?}");
                        }
                    }
                });
            },
            HandlerCommand::FindNode { target, tx } => {
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
            HandlerCommand::Shutdown => self.received_shutdown = true,
        }
        Ok(())
    }

    fn handle_incoming(&mut self, datagram: Vec<u8>, address: SocketAddr) -> Result<()> {
        let message: Message = bincode::deserialize(datagram.as_slice())?;
        match message.ty {
            MessageType::Query => {
                tokio::spawn(handle_query(
                    self.table_tx.clone(),
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
                        let sender_key = self.local_key;
                        tokio::spawn(async move {
                            if event_tx
                                .send(ResponseEvent {
                                    id: message.token,
                                    sender_key,
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
