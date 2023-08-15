use std::{path::Path, sync::Arc};

use affair::Socket;
use async_trait::async_trait;
use fleek_crypto::{
    ConsensusPublicKey, ConsensusSecretKey, NodePublicKey, NodeSecretKey, NodeSignature,
};
use tokio::sync::Notify;

use crate::{
    application::SyncQueryRunnerInterface, config::ConfigConsumer, consensus::MempoolSocket,
    types::UpdateMethod, WithStartAndShutdown,
};

/// A socket that is responsible to submit a transaction to the consensus from our node,
/// implementation of this socket needs to assure the consistency and increment of the
/// nonce (which we also refer to as the counter).
pub type SubmitTxSocket = Socket<UpdateMethod, u64>;

/// The signature provider is responsible for signing messages using the private key of
/// the node.
#[async_trait]
pub trait SignerInterface: ConfigConsumer + WithStartAndShutdown + Sized + Send + Sync {
    // -- DYNAMIC TYPES

    type SyncQuery: SyncQueryRunnerInterface;

    // -- BOUNDED TYPES
    // empty

    /// Initialize the signature service.
    fn init(config: Self::Config, query_runner: Self::SyncQuery) -> anyhow::Result<Self>;

    /// Provide the signer service with the mempool socket after initialization, this function
    /// should only be called once.
    fn provide_mempool(&mut self, mempool: MempoolSocket);

    // Provide the signer service with a block notifier to get notified when a block of
    // transactions has been processed at the application.
    fn provide_new_block_notify(&self, block_notify: Arc<Notify>);

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
