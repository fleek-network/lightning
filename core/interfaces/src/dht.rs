use std::sync::Arc;

use affair::Socket;
use async_trait::async_trait;
use infusion::infu;

use crate::{
    infu_collection::Collection,
    types::{DhtRequest, DhtResponse},
    ConfigConsumer, SignerInterface, TopologyInterface, WithStartAndShutdown,
};

pub type DhtSocket = Socket<DhtRequest, DhtResponse>;

/// The distributed hash table for Fleek Network.
#[async_trait]
pub trait DhtInterface: ConfigConsumer, WithStartAndShutdown + Sized + Send + Sync {
    infu!(DhtInterface @ Input);

    // -- DYNAMIC TYPES

    type Topology: TopologyInterface;

    // -- BOUNDED TYPES
    // empty

    fn init<S: SignerInterface>(
        signer: &S,
        topology: Arc<Self::Topology>,
        config: Self::Config,
    ) -> anyhow::Result<Self>;

    fn get_socket(&self) -> DhtSocket;
}
