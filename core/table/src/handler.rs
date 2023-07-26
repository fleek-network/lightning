use std::{collections::HashSet, net::SocketAddr};

use anyhow::Result;
use fleek_crypto::NodeNetworkingPublicKey;
use tokio::{net::UdpSocket, select, sync::mpsc::Receiver};

use crate::{
    query::{Command, Message, MessagePayload, NodeInfo, Query, Response},
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
            Command::Get { .. } => todo!(),
            Command::Put { .. } => todo!(),
        }
    }

    async fn handle_incoming_datagram(&self, datagram: Vec<u8>, address: SocketAddr) -> Result<()> {
        let message: Message = bincode::deserialize(datagram.as_slice())?;
        let message_id = message.id;
        match message.payload {
            MessagePayload::Query(query) => match query {
                Query::FindNode { key, .. } => {
                    let nodes = self.handle_find_node(&key).await?;
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

    async fn handle_find_node(&self, target: &NodeNetworkingPublicKey) -> Result<Vec<NodeInfo>> {
        Ok(self.table.closest_nodes(target))
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
