use std::borrow::BorrowMut;
use std::sync::Arc;

use aya::maps::{HashMap, MapData};
use common::IpPortKey;
use tokio::net::UnixStream;
use tokio::sync::Mutex;

pub struct Connection<T> {
    socket: UnixStream,
    block_list: Arc<Mutex<HashMap<T, IpPortKey, u32>>>,
}

impl<T: BorrowMut<MapData>> Connection<T> {
    pub fn new(socket: UnixStream, block_list: Arc<Mutex<HashMap<T, IpPortKey, u32>>>) -> Self {
        Self { socket, block_list }
    }

    pub async fn handle(self) -> anyhow::Result<()> {
        todo!()
    }
}
