use affair::Socket;
use anyhow::Result;
use fdi::BuildGraph;
use lightning_types::{PeerRequestError, ServerRequest, ServerResponse};
use tokio::sync::broadcast;

use crate::components::NodeComponents;
use crate::ConfigConsumer;

pub type BlockstoreServerSocket =
    Socket<ServerRequest, broadcast::Receiver<Result<ServerResponse, PeerRequestError>>>;

#[interfaces_proc::blank]
pub trait BlockstoreServerInterface<C: NodeComponents>:
    BuildGraph + Sized + Send + Sync + ConfigConsumer
{
    #[socket]
    fn get_socket(&self) -> BlockstoreServerSocket;
}
