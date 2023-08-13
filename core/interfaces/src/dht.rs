use std::sync::Arc;

use affair::Socket;
use async_trait::async_trait;

use crate::{
    types::{DhtRequest, DhtResponse},
    SignerInterface, TopologyInterface, WithStartAndShutdown,
};

pub type DhtSocket = Socket<DhtRequest, DhtResponse>;

/// The distributed hash table for Fleek Network.
#[async_trait]
pub trait DhtInterface: WithStartAndShutdown + Sized + Send + Sync {
    // -- DYNAMIC TYPES

    type Topology: TopologyInterface;

    // -- BOUNDED TYPES
    // empty

    fn init<S: SignerInterface>(signer: &S, topology: Arc<Self::Topology>) -> anyhow::Result<Self>;

    fn get_socket(&self) -> DhtSocket;
}
