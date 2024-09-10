use std::net::SocketAddr;
use std::path::PathBuf;

use fdi::BuildGraph;
use ready::ReadyWaiterState;
use tokio::sync::broadcast;

use crate::collection::Collection;
use crate::types::Event;
use crate::ConfigConsumer;

/// A wrapper around a tokio broadcast
#[derive(Debug)]
pub struct Events(broadcast::Sender<Vec<Event>>);

impl Clone for Events {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl From<broadcast::Sender<Vec<Event>>> for Events {
    fn from(sender: broadcast::Sender<Vec<Event>>) -> Self {
        Self(sender)
    }
}

impl Events {
    pub fn send(&self, event: Vec<Event>) {
        // Will error if there are no exisiting receivers, however we dont care about that
        let _ = self.0.send(event);
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Vec<Event>> {
        self.0.subscribe()
    }
}

/// The interface for the *RPC* server. Which is supposed to be opening a public
/// port (possibly an HTTP server) and accepts queries or updates from the user.
#[interfaces_proc::blank]
pub trait RpcInterface<C: Collection>: ConfigConsumer + BuildGraph + Sized + Send + Sync {
    type ReadyState: ReadyWaiterState;

    /// Panics if the event handler is not available.
    fn event_tx(&self) -> Events;

    fn port(config: &<Self as ConfigConsumer>::Config) -> u16;

    fn hmac_secret_dir(config: &<Self as ConfigConsumer>::Config) -> Option<PathBuf>;

    /// Wait for the server to be ready after starting.
    async fn wait_for_ready(&self) -> Self::ReadyState;

    /// Get the listen address of the RPC server.
    fn listen_address(&self) -> Option<SocketAddr>;
}
