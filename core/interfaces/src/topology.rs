use std::sync::Arc;

use fleek_crypto::NodePublicKey;
use infusion::c;
use tokio::sync::watch;

use crate::infu_collection::Collection;
use crate::{
    ApplicationInterface,
    ConfigConsumer,
    ConfigProviderInterface,
    KeystoreInterface,
    NotifierInterface,
    WithStartAndShutdown,
};

/// The algorithm used for clustering our network and dynamically creating a network topology.
/// This clustering is later used in other parts of the codebase when connection to other nodes
/// is required. The gossip layer is an example of a component that can feed the data this
/// algorithm generates.
#[infusion::service]
pub trait TopologyInterface<C: Collection>:
    WithStartAndShutdown + ConfigConsumer + Sized + Send + Sync + Clone
{
    fn _init(
        config: ::ConfigProviderInterface,
        signer: ::KeystoreInterface,
        notifier: ::NotifierInterface,
        app: ::ApplicationInterface,
    ) {
        Self::init(
            config.get::<Self>(),
            signer.get_ed25519_pk(),
            notifier.clone(),
            app.sync_query(),
        )
    }

    /// Create an instance of the structure from the provided configuration and public key.
    /// The public key is supposed to be the public key of our own node. This can be obtained
    /// from a [Signer](crate::SignerInterface). But making the `TopologyInterface` depend on
    /// SignerInterface seems odd and we want to avoid that.
    ///
    /// Due to that reason instead we just pass the public key here. For consistency and
    /// correctness of the implementation it is required that this public key to be our
    /// actual public key which is obtained from [get_bls_pk](crate::SignerInterface::get_bls_pk).
    fn init(
        config: Self::Config,
        our_public_key: NodePublicKey,
        notifier: C::NotifierInterface,
        query_runner: c!(C::ApplicationInterface::SyncExecutor),
    ) -> anyhow::Result<Self>;

    /// Get a receiver that will periodically receive the new list of connections that our current
    /// node must connect to. This list will be sent after the epoch changes, but can also be sent
    /// more frequently.
    ///
    /// The list of connections is a 2-dimensional array, the first dimension determines the
    /// closeness of the nodes, the further items are the outer layer of the connections.
    fn get_receiver(&self) -> watch::Receiver<Arc<Vec<Vec<NodePublicKey>>>>;
}
