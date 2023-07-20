use thiserror::Error;

use super::{Connection, RemoteAddr};
use crate::state::with_node;

#[derive(Debug, Clone, Copy, Error)]
pub enum ConnectError {
    #[error("Remote is down.")]
    RemoteIsDown,
    #[error("Unknown connection error.")]
    Unknown,
}

/// Create a connection to the given remote peer on the provided port.
pub async fn connect(remote: RemoteAddr, port: u16) -> Result<Connection, ConnectError> {
    let (rid, remote_rid_fut) = with_node(|n| n.connect(remote, port));
    let remote_rid = remote_rid_fut.await?;
    Ok(Connection::new(rid, remote, remote_rid))
}
