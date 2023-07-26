use std::net::SocketAddr;

use draco_interfaces::Blake3Hash;
use fleek_crypto::NodeNetworkingPublicKey;
use serde::{Deserialize, Serialize};

pub type TableHash = Blake3Hash;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct NodeInfo {
    pub address: SocketAddr,
    pub key: NodeNetworkingPublicKey,
}

#[derive(Debug, Deserialize, Serialize)]
pub enum Query {
    FindNode {
        key: NodeNetworkingPublicKey,
        target: NodeNetworkingPublicKey,
    },
    Find {
        key: NodeNetworkingPublicKey,
        target: TableHash,
    },
    Store {
        key: Vec<u8>,
        value: Vec<u8>,
    },
    Ping,
}

#[derive(Debug, Deserialize, Serialize)]
pub enum MessagePayload {
    Query(Query),
    Response(Response),
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Message {
    pub id: u64,
    pub payload: MessagePayload,
}

#[derive(Debug, Deserialize, Serialize)]
pub enum Response {
    NodeInfo(Vec<NodeInfo>),
    Pong,
}

#[derive(Debug)]
pub enum Command {
    Get { key: Vec<u8> },
    Put { key: Vec<u8>, value: Vec<u8> },
}
