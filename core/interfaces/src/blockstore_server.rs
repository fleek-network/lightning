use affair::Socket;
use anyhow::Result;
use fdi::BuildGraph;
use lightning_types::{PeerRequestError, ServerRequest};
use tokio::sync::broadcast;

use crate::collection::Collection;
use crate::ConfigConsumer;

pub type BlockstoreServerSocket =
    Socket<ServerRequest, broadcast::Receiver<Result<(), PeerRequestError>>>;

#[interfaces_proc::blank]
pub trait BlockstoreServerInterface<C: Collection>:
    BuildGraph + Sized + Send + Sync + ConfigConsumer
{
    #[blank = Socket::raw_bounded(32).0]
    fn get_socket(&self) -> BlockstoreServerSocket;
}
