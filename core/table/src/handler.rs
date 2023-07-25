use std::{collections::HashSet, net::SocketAddr};

use anyhow::Result;
use fleek_crypto::NodeNetworkingPublicKey;
use tokio::{net::UdpSocket, select, sync::mpsc::Receiver};

use crate::{
    query::{Command, Message, NodeInfo, Query, Response},
    routing::Table,
};

pub struct Handler {
    socket: UdpSocket,
    /// Inflight messages.
    inflight: HashSet<u64>,
    command_receiver: Receiver<Command>,
    table: Table,
}

impl Handler {
    pub async fn run(mut self) {
        loop {
            select! {
                command = self.command_receiver.recv() => {
                    match command {
                        Some(command) => self.handle_command(command).await,
                        None => break,
                    }
                }
                incoming = recv_from(&self.socket) => {
                    match incoming {
                        Ok((datagram, address)) => {
                            if let Err(e) = self.handle_incoming_datagram(datagram, address).await {
                                tracing::error!("unexpected error when handling incoming message: {e:?}")
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

    async fn handle_command(&self, command: Command) {
        match command {
            Command::Get => {},
            Command::Put => {},
        }
    }

    async fn handle_incoming_datagram(&self, datagram: Vec<u8>, address: SocketAddr) -> Result<()> {
        let message: Message = bincode::deserialize(datagram.as_slice())?;
        match message {
            Message::Query { id, payload, .. } => match payload {
                Query::FindNode { key } => {
                    let nodes = self.handle_find_node(&key).await?;
                    let query = Message::Response {
                        id,
                        payload: Response::NodeInfo(nodes),
                    };
                    let bytes = bincode::serialize(&query)?;
                    send_to(&self.socket, bytes.as_slice(), address).await?;
                },
                Query::Store { key, value } => {
                    self.handle_store(key, value).await?;
                },
                Query::Ping => {
                    let query = Message::Response {
                        id,
                        payload: Response::Pong,
                    };
                    let bytes = bincode::serialize(&query)?;
                    send_to(&self.socket, bytes.as_slice(), address).await?;
                },
            },
            Message::Response { .. } => {},
        }
        Ok(())
    }

    async fn handle_find_node(&self, target: &NodeNetworkingPublicKey) -> Result<Vec<NodeInfo>> {
        Ok(self.table.closest_nodes(target))
    }

    async fn handle_store(&self, _: Vec<u8>, _: Vec<u8>) -> Result<()> {
        todo!()
    }
}

async fn recv_from(socket: &UdpSocket) -> std::io::Result<(Vec<u8>, SocketAddr)> {
    let mut buf = vec![0u8; 64 * 1024];
    let (size, address) = socket.recv_from(&mut buf).await?;
    buf.truncate(size);
    Ok((buf, address))
}

async fn send_to(socket: &UdpSocket, buf: &[u8], peer: SocketAddr) -> std::io::Result<()> {
    socket.send_to(buf, peer).await?;
    Ok(())
}
