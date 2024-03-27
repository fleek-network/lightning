use log::{error, info};
use tokio::net::UnixListener;

use crate::connection::Connection;
use crate::state::SharedState;

pub struct Server {
    listener: UnixListener,
    shared_state: SharedState,
}

impl Server {
    pub fn new(listener: UnixListener, shared_state: SharedState) -> Self {
        Self {
            listener,
            shared_state,
        }
    }

    pub async fn start(self) -> anyhow::Result<()> {
        loop {
            match self.listener.accept().await {
                Ok((stream, _addr)) => {
                    let shared_state = self.shared_state.clone();
                    tokio::spawn(async move {
                        if let Err(e) = Connection::new(stream, shared_state).handle().await {
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
