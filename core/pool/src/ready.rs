use std::net::SocketAddr;

use ready::tokio::TokioReadyWaiter;

pub type PoolReadyWaiter = TokioReadyWaiter<PoolReadyState>;

#[derive(Debug, Default, Clone)]
pub struct PoolReadyState {
    pub listen_address: Option<SocketAddr>,
}
