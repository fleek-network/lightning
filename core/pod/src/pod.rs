use std::marker::PhantomData;
use std::sync::Arc;

use affair::AsyncWorker;
use fleek_crypto::{NodePublicKey, NodeSecretKey};
use lightning_interfaces::prelude::*;
use lightning_interfaces::types::PodRequest;
use lightning_interfaces::{spawn_worker, PodInterface, PodSocket};
use tokio::sync::Mutex;
use tracing::debug;

use crate::{runner, Config};

#[allow(unused)]
pub struct Pod<C: NodeComponents> {
    socket: PodSocket,
    worker: PodWorker<C>,
    _c: PhantomData<C>,
}

#[derive(Clone)]
struct PodWorker<C: NodeComponents> {
    state: Arc<Mutex<PodState<C>>>,
}

#[allow(unused)]
struct PodState<C: NodeComponents> {
    query_runner: c![C::ApplicationInterface::SyncExecutor],
    node_secret_key: NodeSecretKey,
    node_public_key: NodePublicKey,
    config: Config,
}

impl<C: NodeComponents> Pod<C> {
    pub fn init(
        config_provider: &C::ConfigProviderInterface,
        keystore: &C::KeystoreInterface,
        app: &C::ApplicationInterface,
        fdi::Cloned(_notifier): fdi::Cloned<C::NotifierInterface>,
        fdi::Cloned(waiter): fdi::Cloned<lightning_interfaces::ShutdownWaiter>,
    ) -> Self {
        let config = config_provider.get::<Self>();
        let query_runner = app.sync_query();

        let state = PodState {
            query_runner,
            node_secret_key: keystore.get_ed25519_sk(),
            node_public_key: keystore.get_ed25519_pk(),
            config,
        };

        let worker = PodWorker {
            state: Arc::new(Mutex::new(state)),
        };

        let socket = spawn_worker!(worker.clone(), "POD", waiter, crucial);

        Self {
            socket,
            worker,
            _c: PhantomData,
        }
    }

    pub async fn start(
        _this: fdi::Ref<Self>,
        notifier: fdi::Ref<C::NotifierInterface>,
        fdi::Cloned(_query_runner): fdi::Cloned<c![C::ApplicationInterface::SyncExecutor]>,
    ) {
        debug!("POD started");
        let _subscriber = notifier.subscribe_block_executed();
        tokio::task::spawn_blocking(move || {
            runner::run();
        });
    }
}

impl<C: NodeComponents> PodInterface<C> for Pod<C> {
    /// Returns a socket that can be used to submit transactions to the mempool, these
    /// transactions are signed by the node and a proper nonce is assigned by the
    /// implementation.
    ///
    /// # Panics
    ///
    /// This function can panic if there has not been a prior call to `provide_mempool`.
    fn get_socket(&self) -> PodSocket {
        self.socket.clone()
    }
}

impl<C: NodeComponents> PodState<C> {
    fn _init_state(&mut self) {}

    async fn handle_request(&mut self, _request: PodRequest) {}
}

impl<C: NodeComponents> AsyncWorker for PodWorker<C> {
    type Request = PodRequest;
    type Response = ();

    async fn handle(&mut self, request: PodRequest) {
        let mut state = self.state.lock().await;
        state.handle_request(request).await;
    }
}

impl<C: NodeComponents> BuildGraph for Pod<C> {
    fn build_graph() -> fdi::DependencyGraph {
        fdi::DependencyGraph::new().with_infallible(
            Self::init.with_event_handler("start", Self::start.wrap_with_block_on()),
        )
    }
}

impl<C: NodeComponents> ConfigConsumer for Pod<C> {
    const KEY: &'static str = "pod";

    type Config = Config;
}
