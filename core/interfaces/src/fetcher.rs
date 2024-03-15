use affair::Socket;
use fdi::BuildGraph;
use lightning_types::{FetcherRequest, FetcherResponse};

use crate::infu_collection::Collection;

pub type FetcherSocket = Socket<FetcherRequest, FetcherResponse>;

#[infusion::service]
pub trait FetcherInterface<C: Collection>: BuildGraph + Sized + Send + Sync {
    /// Returns a socket that can be used to submit requests to the fetcher.
    #[blank = FetcherSocket::raw_bounded(1).0]
    fn get_socket(&self) -> FetcherSocket;
}
