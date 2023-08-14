use affair::Socket;
use infusion::{infu, p};

use crate::{
    infu_collection::Collection,
    types::{DhtRequest, DhtResponse},
    ConfigProviderInterface, ConfigConsumer, SignerInterface, TopologyInterface,
    WithStartAndShutdown,
};

pub type DhtSocket = Socket<DhtRequest, DhtResponse>;

/// The distributed hash table for Fleek Network.
#[infusion::blank]
pub trait DhtInterface: ConfigConsumer + WithStartAndShutdown + Sized + Send + Sync {
    infu!(DhtInterface, {
        fn init(signer: SignerInterface, topology: TopologyInterface, config: ConfigProviderInterface) {
            Self::init(signer, topology.clone(), config.get::<Self>())
        }
    });

    fn init(
        signer: &p!(::SignerInterface),
        topology: p!(::TopologyInterface),
        config: Self::Config,
    ) -> anyhow::Result<Self>;

    fn get_socket(&self) -> DhtSocket;
}
