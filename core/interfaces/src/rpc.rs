use fdi::BuildGraph;
use tokio::sync::mpsc;

use crate::collection::Collection;
use crate::types::Event;

/// The interface for the *RPC* server. Which is supposed to be opening a public
/// port (possibly an HTTP server) and accepts queries or updates from the user.
#[interfaces_proc::blank]
pub trait RpcInterface<C: Collection>: BuildGraph + Sized + Send + Sync {
    fn event_tx(&self) -> mpsc::Sender<Vec<Event>>;
}
