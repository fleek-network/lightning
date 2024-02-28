use affair::Socket;
use anyhow::Result;
use fleek_crypto::ConsensusPublicKey;
use infusion::{c, service};
use lightning_types::TransactionRequest;

use crate::infu_collection::Collection;
use crate::{ApplicationInterface, ConfigConsumer, ConfigProviderInterface, KeystoreInterface};

/// A socket that gives services and other sub-systems the required functionality to
/// submit messages/transactions to the consensus.
///
/// # Safety
///
/// This socket is safe to freely pass around, sending transactions through this socket
/// does not guarantee their execution on the application layer. You can think about
/// this as if the current node was only an external client to the network.
pub type MempoolSocket = Socket<TransactionRequest, ()>;

#[service]
pub trait ForwarderInterface<C: Collection>: ConfigConsumer + Sized + Send + 'static {
    fn _init(
        provider: ::ConfigProviderInterface,
        app: ::ApplicationInterface,
        keystore: ::KeystoreInterface,
    ) {
        Self::init(
            provider.get::<Self>(),
            keystore.get_bls_pk(),
            app.sync_query(),
        )
    }

    fn init(
        config: Self::Config,
        consensus_key: ConsensusPublicKey,
        query_runner: c!(C::ApplicationInterface::SyncExecutor),
    ) -> Result<Self>;

    /// Get the socket for forwarding new transaction requests to the mempool.
    #[blank = Socket::raw_bounded(64).0]
    fn mempool_socket(&self) -> MempoolSocket;
}
