use std::{collections::HashSet, net::SocketAddr};

use anyhow::Result;
use fleek_crypto::NodeNetworkingPublicKey;
use tokio::{net::UdpSocket, select, sync::mpsc::Receiver};

use crate::query::{Command, Message, Query, Response};

pub struct Handler {
    socket: UdpSocket,
    /// Inflight messages.
    inflight: HashSet<u64>,
    command_receiver: Receiver<Command>,
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
                incoming = recv_from(&mut self.socket) => {
                    match incoming {
                        Ok((datagram, address)) => self.handle_datagram(datagram, address).await.unwrap(),
                        Err(_) => {
                            // Todo: Log error.
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
            Command::FindNode { .. } => {},
            Command::Store => {},
        }
    }

    async fn handle_datagram(&self, datagram: Vec<u8>, address: SocketAddr) -> Result<()> {
        let message: Message = bincode::deserialize(datagram.as_slice())?;
        match message {
            Message::Query { payload, .. } => match payload {
                Query::FindNode { .. } => {
                    let query = Message::Response {
                        id: 0,
                        // Todo: Update.
                        payload: Response::Pong,
                    };
                    let bytes = match bincode::serialize(&query) {
                        Ok(bytes) => bytes,
                        Err(_) => {
                            // Todo: Log error.
                            todo!()
                        },
                    };
                    send_to(&self.socket, bytes.as_slice(), address);
                },
                Query::Store { .. } => {
                    let query = Message::Response {
                        id: 0,
                        // Todo: Update.
                        payload: Response::Pong,
                    };
                    let bytes = match bincode::serialize(&query) {
                        Ok(bytes) => bytes,
                        Err(_) => {
                            // Todo: Log error.
                            todo!()
                        },
                    };
                    send_to(&self.socket, bytes.as_slice(), address);
                },
                Query::Ping => {
                    let query = Message::Response {
                        id: 0,
                        payload: Response::Pong,
                    };
                    let bytes = match bincode::serialize(&query) {
                        Ok(bytes) => bytes,
                        Err(_) => {
                            // Todo: Log error.
                            todo!()
                        },
                    };
                    send_to(&self.socket, bytes.as_slice(), address);
                },
            },
            Message::Response { .. } => {},
        }
        Ok(())
    }

    async fn handle_find_node(&self, _: NodeNetworkingPublicKey) {
        todo!()
    }

    async fn handle_store(&self, _: Vec<u8>, _: Vec<u8>) {
        todo!()
    }

    async fn handle_ping() {
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
