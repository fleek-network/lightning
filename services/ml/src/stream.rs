use fn_sdk::connection::Connection;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Data {
    origin: Origin,
    uri: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[repr(u8)]
pub enum Origin {
    Blake3,
    Ipfs,
    Http,
    Unknown,
}

#[allow(dead_code)]
pub struct ServiceStream {
    connection: Connection,
}
