use std::{
    collections::HashSet,
    net::SocketAddr,
    sync::{Arc, Mutex},
};

use anyhow::{bail, Result};
use fleek_crypto::NodeNetworkingPublicKey;
use tokio::{
    net::UdpSocket,
    select,
    sync::{
        mpsc::{Receiver, Sender},
        oneshot,
    },
    task,
};

use crate::{
    query::{Message, MessagePayload, NodeInfo, Query, Response, ValueHash},
    table::{self, TableQuery},
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
}

pub async fn start_server(
    mut command_rx: Receiver<Command>,
    socket: Arc<UdpSocket>,
    local_key: NodeNetworkingPublicKey,
) {
    // Todo: Make configurable.
    let (table_tx, table_rx) = tokio::sync::mpsc::channel(10000);
    task::spawn(table::start_server(table_rx, local_key));
    let main_hanlder = Handler::new(socket.clone(), table_tx);
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
            incoming = recv_from(&socket) => {
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
    pub fn new(socket: Arc<UdpSocket>, table_tx: Sender<TableQuery>) -> Self {
        Self {
            socket,
            _inflight: Arc::new(Default::default()),
            table_tx,
        }
    }

    async fn handle_command(&self, command: Command) -> Result<()> {
        match command {
            Command::Get { key, tx } => {
                let hash = match ValueHash::try_from(key) {
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
            Command::Put { .. } => todo!(),
        }
        Ok(())
    }

    async fn handle_incoming_datagram(&self, datagram: Vec<u8>, address: SocketAddr) -> Result<()> {
        let message: Message = bincode::deserialize(datagram.as_slice())?;
        let message_id = message.id;
        match message.payload {
            MessagePayload::Query(query) => match query {
                Query::FindNode { key, .. } => {
                    let nodes = self.closest_nodes(&key).await?;
                    let query = Message {
                        id: message_id,
                        payload: MessagePayload::Response(Response::NodeInfo(nodes)),
                    };
                    let bytes = bincode::serialize(&query)?;
                    send_to(&self.socket, bytes.as_slice(), address).await?;
                },
                Query::Find { .. } => {
                    todo!()
                },
                Query::Store { .. } => {
                    todo!()
                },
                Query::Ping => {
                    let query = Message {
                        id: message_id,
                        payload: MessagePayload::Response(Response::Pong),
                    };
                    let bytes = bincode::serialize(&query)?;
                    send_to(&self.socket, bytes.as_slice(), address).await?;
                },
            },
            MessagePayload::Response(_) => {},
        }
        Ok(())
    }

    async fn closest_nodes(&self, target: &NodeNetworkingPublicKey) -> Result<Vec<NodeInfo>> {
        let (tx, rx) = oneshot::channel();
        if self
            .table_tx
            .send(TableQuery::ClosestNodes { key: *target, tx })
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

    async fn look_up(&self, target: ValueHash) -> Result<()> {
        todo!()
    }

    async fn find_value(&self, _: ValueHash) -> Result<Vec<u8>> {
        todo!()
    }
}

async fn recv_from(socket: &UdpSocket) -> std::io::Result<(Vec<u8>, SocketAddr)> {
    // Todo: Let's make sure that our messages can fit in one datagram.
    let mut buf = vec![0u8; 64 * 1024];
    let (size, address) = socket.recv_from(&mut buf).await?;
    buf.truncate(size);
    Ok((buf, address))
}

async fn send_to(socket: &UdpSocket, buf: &[u8], peer: SocketAddr) -> std::io::Result<()> {
    socket.send_to(buf, peer).await?;
    Ok(())
}
