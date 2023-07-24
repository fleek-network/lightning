use super::Connection;
use crate::state::with_node;

/// A connection listener that allows the node to accept connections on a certain port.
pub struct Listener {
    port: u16,
}

impl Listener {
    /// Accept a new connection. Returns `None` if there is no more connection coming.
    pub async fn accept(&mut self) -> Option<Connection> {
        let r = with_node(|n| n.accept(self.port)).await?;
        Some(Connection::new(r.local_rid, r.remote, r.remote_rid))
    }
}

impl Drop for Listener {
    fn drop(&mut self) {
        with_node(|n| n.close_listener(self.port));
    }
}

/// Start a listener on the given port.
///
/// # Panics
///
/// If the current port is already being listened on by another listener.
pub fn listen(port: u16) -> Listener {
    with_node(|n| n.listen(port));
    Listener { port }
}
