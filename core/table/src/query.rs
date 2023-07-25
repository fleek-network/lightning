use std::net::SocketAddr;

use fleek_crypto::NodeNetworkingPublicKey;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct NodeInfo {
    pub address: SocketAddr,
    pub key: NodeNetworkingPublicKey,
}

#[derive(Debug, Deserialize, Serialize)]
pub enum Query {
    FindNode { key: NodeNetworkingPublicKey },
    Store { key: Vec<u8>, value: Vec<u8> },
    Ping,
}

#[derive(Debug, Deserialize, Serialize)]
pub enum Message {
    Query { id: u64, payload: Query },
    Response { id: u64, payload: Response },
}

#[derive(Debug, Deserialize, Serialize)]
pub enum Response {
    NodeInfo(Vec<NodeInfo>),
    Pong,
}

#[derive(Debug)]
pub enum Command {
    Get,
    Put,
}
