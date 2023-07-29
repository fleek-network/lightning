use std::net::SocketAddr;

use fleek_crypto::NodeNetworkingPublicKey;
use serde::{Deserialize, Serialize};

use crate::table::TableKey;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct NodeInfo {
    pub address: SocketAddr,
    pub key: NodeNetworkingPublicKey,
}

#[derive(Debug, Deserialize, Serialize)]
pub enum Query {
    Find {
        find_value: bool,
        sender_id: NodeNetworkingPublicKey,
        target: TableKey,
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
pub struct Response {
    pub sender_id: TableKey,
    pub nodes: Vec<NodeInfo>,
    pub breadcrumb: u64,
    pub value: Option<Vec<u8>>,
}
