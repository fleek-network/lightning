use std::sync::Arc;

use aya::maps::{HashMap, MapData};
use common::IpPortKey;
use log::{error, info};
use tokio::net::UnixListener;
use tokio::sync::Mutex;

use crate::connection::Connection;

pub struct Server {
    listener: UnixListener,
    block_list: Arc<Mutex<HashMap<MapData, IpPortKey, u32>>>,
}

impl Server {
    pub fn new(listener: UnixListener, block_list: HashMap<MapData, IpPortKey, u32>) -> Self {
        Self {
            listener,
            block_list: Arc::new(Mutex::new(block_list)),
        }
    }

    pub async fn start(self) -> anyhow::Result<()> {
        loop {
            match self.listener.accept().await {
                Ok((stream, _addr)) => {
                    let block_list = self.block_list.clone();
                    tokio::spawn(async move {
                        if let Err(e) = Connection::new(stream, block_list).handle().await {
                            info!("connection handler failed: {e:?}");
                        }
                    });
                },
                Err(e) => {
                    error!("accept failed: {e:?}")
                },
            }
        }
    }
}
