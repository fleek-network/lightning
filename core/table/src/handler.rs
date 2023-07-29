use std::{
    collections::{HashMap, HashSet},
    net::SocketAddr,
    sync::{Arc, Mutex, RwLock},
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
    lookup::{LookupHandle, LookupTask},
    query::{Message, MessagePayload, NodeInfo, Query, Response},
    socket,
    socket::send_to,
    table::{self, TableKey, TableQuery},
};

#[derive(Debug)]
pub enum Command {
    Get {
        key: Vec<u8>,
        tx: oneshot::Sender<Result<Vec<u8>, ()>>,
    },
    Put {
        key: Vec<u8>,
        value: Vec<u8>,
    },
}

#[derive(Clone)]
pub struct Handler {
    socket: Arc<UdpSocket>,
    /// Inflight messages.
    _inflight: Arc<Mutex<HashSet<u64>>>,
    table_tx: Sender<TableQuery>,
    node_id: NodeNetworkingPublicKey,
    pending_lookups: Arc<RwLock<HashMap<u64, LookupHandle>>>,
}

pub async fn start_server(
    mut command_rx: Receiver<Command>,
    socket: Arc<UdpSocket>,
    local_key: NodeNetworkingPublicKey,
) {
    // Todo: Make configurable.
    let (table_tx, table_rx) = tokio::sync::mpsc::channel(10000);
    task::spawn(table::start_server(table_rx, local_key));
    let main_hanlder = Handler::new(socket.clone(), table_tx, local_key);
    loop {
        let handler = main_hanlder.clone();
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

impl Handler {
    pub fn new(
        socket: Arc<UdpSocket>,
        table_tx: Sender<TableQuery>,
        node_id: NodeNetworkingPublicKey,
    ) -> Self {
        Self {
            socket,
            _inflight: Arc::new(Default::default()),
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
                match self._find_value(hash).await {
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
                let nodes = self._find_node(table_key).await?;
                // Todo: Add sender information in message.
                let message = Message {
                    id: 0,
                    payload: MessagePayload::Query(Query::Store { key, value }),
                };
                let bytes = bincode::serialize(&message)?;
                for node in nodes {
                    send_to(&self.socket, &bytes, node.address).await?;
                }
            },
        }
        Ok(())
    }

    async fn handle_incoming_datagram(&self, datagram: Vec<u8>, address: SocketAddr) -> Result<()> {
        let message: Message = bincode::deserialize(datagram.as_slice())?;
        let message_id = message.id;
        match message.payload {
            MessagePayload::Query(query) => match query {
                Query::Find { sender_id: key, .. } => {
                    let nodes = self.closest_nodes(&key).await?;
                    let query = Message {
                        id: message_id,
                        payload: MessagePayload::Response(Response {
                            sender_id: key.0,
                            nodes,
                            breadcrumb: 0,
                        }),
                    };
                    let bytes = bincode::serialize(&query)?;
                    socket::send_to(&self.socket, bytes.as_slice(), address).await?;
                },
                Query::Store { .. } => {
                    todo!()
                },
                Query::Ping => {
                    let query = Message {
                        id: message_id,
                        payload: MessagePayload::Response(Response {
                            sender_id: self.node_id.0,
                            nodes: Default::default(),
                            breadcrumb: 0,
                        }),
                    };
                    let bytes = bincode::serialize(&query)?;
                    socket::send_to(&self.socket, bytes.as_slice(), address).await?;
                },
            },
            MessagePayload::Response(response) => {
                let task_tx = match self
                    .pending_lookups
                    .read()
                    .unwrap()
                    .get(&response.breadcrumb)
                {
                    None => {
                        tracing::warn!("received unsolicited response");
                        return Ok(());
                    },
                    Some(handle) => handle.0.clone(),
                };
                if task_tx.send(response).await.is_err() {
                    tracing::error!("failed to send response to task")
                }
            },
        }
        Ok(())
    }

    async fn closest_nodes(&self, target: &NodeNetworkingPublicKey) -> Result<Vec<NodeInfo>> {
        let (tx, rx) = oneshot::channel();
        if self
            .table_tx
            .send(TableQuery::ClosestNodes {
                target: target.0,
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

    async fn _find_node(&self, target: TableKey) -> Result<Vec<NodeInfo>> {
        let (task_tx, task_rx) = mpsc::channel(1000000);
        // Todo: Randomly generate id/breadcrumb.
        let task_id = 0;
        let task = LookupTask::new(
            task_id,
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
            .insert(task_id, handle);
        lookup::lookup(task).await.map_err(Into::into)
    }

    async fn _find_value(&self, _: TableKey) -> Result<Vec<u8>> {
        todo!()
    }
}
