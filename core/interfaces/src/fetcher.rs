use affair::Socket;
use fdi::BuildGraph;
use lightning_types::{FetcherRequest, FetcherResponse};

use crate::collection::Collection;

pub type FetcherSocket = Socket<FetcherRequest, FetcherResponse>;

#[interfaces_proc::blank]
pub trait FetcherInterface<C: Collection>: BuildGraph + Sized + Send + Sync {
    /// Returns a socket that can be used to submit requests to the fetcher.
    fn get_socket(&self) -> FetcherSocket;
}
