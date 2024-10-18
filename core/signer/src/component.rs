use std::marker::PhantomData;

use affair::AsyncWorker;
use anyhow::Result;
use lightning_interfaces::prelude::*;
use lightning_interfaces::spawn_worker;
use lightning_utils::transaction::{TransactionClient, TransactionSigner};

use crate::worker::SignerWorker;

pub struct Signer<C: NodeComponents> {
    socket: SignerSubmitTxSocket,
    worker: SignerWorker<C>,
    _components: PhantomData<C>,
}

/// The signer is a top-level node component, so it must implement `BuildGraph` to
/// integrate with the node's dependency injection framework.
impl<C: NodeComponents> BuildGraph for Signer<C> {
    fn build_graph() -> fdi::DependencyGraph {
        fdi::DependencyGraph::new().with(
            Self::init
                .with_event_handler("start", Self::start.wrap_with_spawn_named("SIGNER: start")),
        )
    }
}

impl<C: NodeComponents> Signer<C> {
    /// Initialize the signer component.
    ///
    /// This method is called once during node initialization.
    fn init(fdi::Cloned(shutdown): fdi::Cloned<ShutdownWaiter>) -> Result<Self> {
        let worker = SignerWorker::new();
        let socket = spawn_worker!(worker.clone(), "SIGNER: worker socket", shutdown, crucial);
        Ok(Self {
            socket,
            worker,
            _components: PhantomData,
        })
    }

    /// Start the signer worker with a transaction client, and wait for shutdown.
    ///
    /// This method is called once during node startup.
    async fn start(
        this: fdi::Ref<Self>,
        forwarder: fdi::Ref<C::ForwarderInterface>,
        fdi::Cloned(keystore): fdi::Cloned<C::KeystoreInterface>,
        fdi::Cloned(notifier): fdi::Cloned<C::NotifierInterface>,
        fdi::Cloned(app_query): fdi::Cloned<c!(C::ApplicationInterface::SyncExecutor)>,
    ) {
        app_query.wait_for_genesis().await;
        tracing::debug!("starting signer");

        let client = TransactionClient::new(
            app_query,
            notifier,
            forwarder.mempool_socket(),
            TransactionSigner::NodeMain(keystore.get_ed25519_sk()),
        )
        .await;

        this.worker.start(client).await;
    }
}

impl<C: NodeComponents> SignerInterface<C> for Signer<C> {
    /// Returns a socket that can be used to submit transactions to the mempool, these
    /// transactions are signed by the node and a proper nonce is assigned by the
    /// implementation.
    fn get_socket(&self) -> SignerSubmitTxSocket {
        self.socket.clone()
    }
}
