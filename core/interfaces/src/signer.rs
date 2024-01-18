use std::path::Path;

use affair::Socket;
use fleek_crypto::{
    ConsensusPublicKey,
    ConsensusSecretKey,
    NodePublicKey,
    NodeSecretKey,
    NodeSignature,
};
use infusion::c;
use tokio::sync::mpsc;

use crate::config::ConfigConsumer;
use crate::consensus::MempoolSocket;
use crate::infu_collection::Collection;
use crate::types::UpdateMethod;
use crate::{
    ApplicationInterface,
    ConfigProviderInterface,
    ConsensusInterface,
    Notification,
    NotifierInterface,
    WithStartAndShutdown,
};

/// A socket that is responsible to submit a transaction to the consensus from our node,
/// implementation of this socket needs to assure the consistency and increment of the
/// nonce (which we also refer to as the counter).
pub type SubmitTxSocket = Socket<UpdateMethod, u64>;

/// The signature provider is responsible for signing messages using the private key of
/// the node.
#[infusion::service]
pub trait SignerInterface<C: Collection>:
    ConfigConsumer + WithStartAndShutdown + Sized + Send + Sync
{
    fn _init(config: ::ConfigProviderInterface, app: ::ApplicationInterface) {
        Self::init(config.get::<Self>(), app.sync_query())
    }

    fn _post(&mut self, c: ::ConsensusInterface, n: ::NotifierInterface) {
        self.provide_mempool(c.mempool());

        let (new_block_tx, new_block_rx) = mpsc::channel(10);

        self.provide_new_block_notify(new_block_rx);

        n.notify_on_new_block(new_block_tx);
    }

    /// Initialize the signature service.
    fn init(
        config: Self::Config,
        query_runner: c!(C::ApplicationInterface::SyncExecutor),
    ) -> anyhow::Result<Self>;

    /// Provide the signer service with the mempool socket after initialization, this function
    /// should only be called once.
    fn provide_mempool(&mut self, mempool: MempoolSocket);

    // Provide the signer service with a new block notifications channel's receiver to get notified
    // when a block of transactions has been processed at the application.
    fn provide_new_block_notify(&mut self, block_notify_rx: mpsc::Receiver<Notification>);

    /// Returns the `BLS` public key of the current node.
    fn get_bls_pk(&self) -> ConsensusPublicKey;

    /// Returns the `Ed25519` (network) public key of the current node.
    fn get_ed25519_pk(&self) -> NodePublicKey;

    /// Returns the loaded secret key material.
    ///
    /// # Safety
    ///
    /// Just like any other function which deals with secret material this function should
    /// be used with the greatest caution.
    fn get_sk(&self) -> (ConsensusSecretKey, NodeSecretKey);

    /// Returns a socket that can be used to submit transactions to the mempool, these
    /// transactions are signed by the node and a proper nonce is assigned by the
    /// implementation.
    ///
    /// # Panics
    ///
    /// This function can panic if there has not been a prior call to `provide_mempool`.
    fn get_socket(&self) -> SubmitTxSocket;

    /// Sign the provided raw digest and return a signature.
    ///
    /// # Safety
    ///
    /// This function is unsafe to use without proper reasoning, which is trivial since
    /// this function is responsible for signing arbitrary messages from other parts of
    /// the system.
    fn sign_raw_digest(&self, digest: &[u8; 32]) -> NodeSignature;

    /// Generates the node secret key.
    ///
    /// # Safety
    ///
    /// This function will return an error if the key already exists.
    fn generate_node_key(path: &Path) -> anyhow::Result<()>;

    /// Generates the consensus secret keys.
    ///
    /// # Safety
    ///
    /// This function will return an error if the key already exists.
    fn generate_consensus_key(path: &Path) -> anyhow::Result<()>;
}
