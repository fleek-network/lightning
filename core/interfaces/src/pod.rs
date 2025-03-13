use affair::Socket;
use fdi::BuildGraph;
use lightning_types::PodRequest;

use crate::components::NodeComponents;

/// A socket that is responsible to submit a client request to the PoD component.
pub type PodSocket = Socket<PodRequest, ()>;

/// The signature provider is responsible for signing messages using the private key of
/// the node.
#[interfaces_proc::blank]
pub trait PodInterface<C: NodeComponents>: BuildGraph + Sized + Send + Sync {
    /// Returns a socket that can be used to submit a client request to the PoD component.
    #[socket]
    fn get_socket(&self) -> PodSocket;
}
