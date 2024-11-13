use std::net::SocketAddr;

use ready::tokio::TokioReadyWaiter;

pub type PingerReadyWaiter = TokioReadyWaiter<PingerReadyState>;

#[derive(Debug, Clone)]
pub struct PingerReadyState {
    pub listen_address: SocketAddr,
}

impl Default for PingerReadyState {
    fn default() -> Self {
        Self {
            listen_address: "0.0.0.0:0".parse().unwrap(),
        }
    }
}
