use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{Arc, RwLock},
};

use anyhow::{bail, Result};
use fleek_crypto::NodeNetworkingPublicKey;
use tokio::{
    net::UdpSocket,
    select,
    sync::{
        mpsc,
        mpsc::{Receiver, Sender},
        oneshot,
    },
    task,
};

use crate::{
    lookup,
    lookup::{LookupHandle, LookupResult, LookupTask},
    query::{Message, MessagePayload, NodeInfo, Query, Response},
    socket,
    table::{self, TableKey, TableQuery},
};

#[derive(Debug)]
pub enum Command {
    Get {
        key: Vec<u8>,
        tx: oneshot::Sender<Result<Option<Vec<u8>>, ()>>,
    },
    Put {
        key: Vec<u8>,
        value: Vec<u8>,
    },
}

pub async fn start_server(
    mut command_rx: Receiver<Command>,
    socket: Arc<UdpSocket>,
    local_key: NodeNetworkingPublicKey,
) {
    // Todo: Make configurable.
    let (table_tx, table_rx) = mpsc::channel(10000);
    task::spawn(table::start_server(table_rx, local_key));
    let main_handler = Handler::new(socket.clone(), table_tx, local_key);
    loop {
        let handler = main_handler.clone();
        select! {
            command = command_rx.recv() => {
                match command {
                    Some(command) => {
                        task::spawn(async move {
                            if let Err(e) = handler.handle_command(command).await {
                                tracing::error!("command request failed: {e:?}")
                            }
                        });
                    }
                    None => break,
                }
            }
            incoming = socket::recv_from(&socket) => {
                match incoming {
                    Ok((datagram, address)) => {
                        task::spawn(async move {
                            if let Err(e) = handler
                            .handle_incoming_datagram(datagram, address).await
                            {
                                tracing::error!("unexpected error when handling incoming message: {e:?}")
                            }
                        });
                    }
                    Err(e) => {
                        tracing::error!("unexpected error when reading from socket: {e:?}")
                    }
                }
            }
        }
    }
}

#[derive(Clone)]
pub struct Handler {
    socket: Arc<UdpSocket>,
    table_tx: Sender<TableQuery>,
    node_id: NodeNetworkingPublicKey,
    pending_lookups: Arc<RwLock<HashMap<u64, LookupHandle>>>,
}

impl Handler {
    pub fn new(
        socket: Arc<UdpSocket>,
        table_tx: Sender<TableQuery>,
        node_id: NodeNetworkingPublicKey,
    ) -> Self {
        Self {
            socket,
            table_tx,
            node_id,
            pending_lookups: Default::default(),
        }
    }

    async fn handle_command(&self, command: Command) -> Result<()> {
        match command {
            Command::Get { key, tx } => {
                let hash = match TableKey::try_from(key) {
                    Ok(hash) => hash,
                    Err(key) => {
                        tracing::error!("invalid key: {key:?}");
                        tx.send(Err(())).unwrap();
                        return Err(anyhow::anyhow!("failed"));
                    },
                };
                match self.find_value(hash).await {
                    Ok(value) => {
                        tx.send(Ok(value)).unwrap();
                    },
                    Err(e) => {
                        tracing::error!("failed to find value: {e:?}");
                        tx.send(Err(())).unwrap();
                    },
                }
            },
            Command::Put { key, value } => {
                let table_key = TableKey::try_from(key.as_slice())?;
                let nodes = self.find_node(table_key).await?;
                // Todo: Add sender information in message.
                let message = Message {
                    id: 0,
                    sender_key: self.node_id,
                    // Todo: Store handlers do not need this.
                    // Maybe we make it optional?
                    channel_id: 0,
                    payload: MessagePayload::Query(Query::Store {
                        key: table_key,
                        value,
                    }),
                };
                let bytes = bincode::serialize(&message)?;
                for node in nodes {
                    socket::send_to(&self.socket, &bytes, node.address).await?;
                }
            },
        }
        Ok(())
    }

    async fn handle_incoming_datagram(&self, datagram: Vec<u8>, address: SocketAddr) -> Result<()> {
        let message: Message = bincode::deserialize(datagram.as_slice())?;
        let message_id = message.id;
        let sender_key = message.sender_key;
        let channel_id = message.channel_id;
        match message.payload {
            MessagePayload::Query(query) => match query {
                Query::Find { find_value, target } => {
                    let value = match find_value {
                        // Todo: Check from local store.
                        true => todo!(),
                        false => None,
                    };
                    let nodes = self.closest_nodes(&target).await?;
                    let query = Message {
                        id: message_id,
                        channel_id,
                        sender_key: self.node_id,
                        payload: MessagePayload::Response(Response { nodes, value }),
                    };
                    let bytes = bincode::serialize(&query)?;
                    socket::send_to(&self.socket, bytes.as_slice(), address).await?;
                },
                Query::Store { .. } => {
                    // Todo: How do we avoid someone sending tons of Store queries.
                    todo!()
                },
                Query::Ping => {
                    let query = Message {
                        id: message_id,
                        channel_id,
                        sender_key: self.node_id,
                        payload: MessagePayload::Response(Response {
                            nodes: Vec::new(),
                            value: None,
                        }),
                    };
                    let bytes = bincode::serialize(&query)?;
                    socket::send_to(&self.socket, bytes.as_slice(), address).await?;
                },
            },
            MessagePayload::Response(response) => {
                let task_tx = match self.pending_lookups.read().unwrap().get(&channel_id) {
                    None => {
                        tracing::warn!("received unsolicited response");
                        return Ok(());
                    },
                    Some(handle) => handle.0.clone(),
                };
                if task_tx.send((sender_key, response)).await.is_err() {
                    tracing::error!("failed to send response to task")
                }
            },
        }
        Ok(())
    }

    async fn closest_nodes(&self, target: &TableKey) -> Result<Vec<NodeInfo>> {
        let (tx, rx) = oneshot::channel();
        if self
            .table_tx
            .send(TableQuery::ClosestNodes {
                target: *target,
                tx,
            })
            .await
            .is_err()
        {
            bail!("failed to send query to Table server");
        }
        match rx.await {
            Ok(nodes) => nodes.map_err(Into::into),
            Err(e) => bail!("{e}"),
        }
    }

    async fn find_node(&self, target: TableKey) -> Result<Vec<NodeInfo>> {
        let (task_tx, task_rx) = mpsc::channel(1000000);
        // Todo: Randomly generate id/breadcrumb. Check if it exists.
        let channel_id = 0;
        let task = LookupTask::new(
            channel_id,
            false,
            self.node_id,
            target,
            self.table_tx.clone(),
            task_rx,
            self.socket.clone(),
        );
        let handle = LookupHandle(task_tx);
        self.pending_lookups
            .write()
            .unwrap()
            .insert(channel_id, handle);
        match lookup::lookup(task)
            .await
            .map_err(Into::<anyhow::Error>::into)?
        {
            LookupResult::Nodes(nodes) => Ok(nodes),
            LookupResult::Value(_) => panic!("we did not request for a value"),
        }
    }

    async fn find_value(&self, target: TableKey) -> Result<Option<Vec<u8>>> {
        let (task_tx, task_rx) = mpsc::channel(1000000);
        // Todo: Randomly generate id/breadcrumb. Check if it exists.
        let channel_id = 0;
        let task = LookupTask::new(
            channel_id,
            true,
            self.node_id,
            target,
            self.table_tx.clone(),
            task_rx,
            self.socket.clone(),
        );
        let handle = LookupHandle(task_tx);
        self.pending_lookups
            .write()
            .unwrap()
            .insert(channel_id, handle);
        match lookup::lookup(task)
            .await
            .map_err(Into::<anyhow::Error>::into)?
        {
            LookupResult::Nodes(_) => panic!("we did not request for a nodes"),
            LookupResult::Value(value) => Ok(value),
        }
    }
}
