use std::marker::PhantomData;
use std::path::PathBuf;

use fxhash::FxHashSet;
use lightning_interfaces::prelude::*;
use lightning_interfaces::types::ServiceId;
use lightning_test_utils::config::LIGHTNING_TEST_HOME_DIR;
use lightning_utils::config::LIGHTNING_HOME_DIR;
use resolved_pathbuf::ResolvedPathBuf;
use serde::{Deserialize, Serialize};
use tokio::net::UnixStream;
use tracing::{error, trace};
use triomphe::Arc;

use crate::service::{spawn_service, Context, ServiceCollection};

#[derive(Clone)]
pub struct ServiceExecutor<C: Collection> {
    config: Arc<ServiceExecutorConfig>,
    collection: ServiceCollection,
    ctx: Arc<Context<C>>,
    p: PhantomData<C>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct ServiceExecutorConfig {
    pub services: FxHashSet<ServiceId>,
    /// The IPC directory is used to contain the Unix domain sockets that we use to communicate
    /// with the different services.
    pub ipc_path: ResolvedPathBuf,
}

impl Default for ServiceExecutorConfig {
    fn default() -> Self {
        Self {
            services: [0, 1].into_iter().collect(),
            ipc_path: LIGHTNING_HOME_DIR
                .join("ipc")
                .try_into()
                .expect("Failed to resolve path"),
        }
    }
}

impl ServiceExecutorConfig {
    pub fn test_default() -> Self {
        Self {
            services: Default::default(),
            ipc_path: LIGHTNING_TEST_HOME_DIR
                .join("ipc")
                .try_into()
                .expect("Failed to resolve path"),
        }
    }
}

#[derive(Clone)]
pub struct Provider {
    ipc_dir: PathBuf,
    collection: ServiceCollection,
}

impl<C: Collection> ServiceExecutor<C> {
    /// Initialize the service executor.
    fn init(
        config: &C::ConfigProviderInterface,
        blockstore: &C::BlockstoreInterface,
        fetcher: &C::FetcherInterface,
        fdi::Cloned(query_runner): fdi::Cloned<c!(C::ApplicationInterface::SyncExecutor)>,
        fdi::Cloned(task_broker): fdi::Cloned<C::TaskBrokerInterface>,
    ) -> anyhow::Result<Self> {
        let config = Arc::new(config.get::<Self>());

        let ctx = Arc::new(Context {
            blockstore_path: blockstore.get_root_dir(),
            ipc_path: config.ipc_path.to_path_buf(),
            fetcher_socket: fetcher.get_socket(),
            query_runner,
            task_broker,
        });

        Ok(ServiceExecutor {
            config,
            collection: ServiceCollection::default(),
            ctx,
            p: PhantomData,
        })
    }

    async fn start(
        fdi::Cloned(this): fdi::Cloned<Self>,
        fdi::Cloned(query_runner): fdi::Cloned<c!(C::ApplicationInterface::SyncExecutor)>,
        fdi::Cloned(waiter): fdi::Cloned<ShutdownWaiter>,
    ) {
        spawn!(
            async move {
                query_runner.wait_for_genesis().await;

                for &id in this.config.services.iter() {
                    let handle = spawn_service(id, this.ctx.clone(), waiter.clone()).await;
                    this.collection.insert(id, handle);
                }
            },
            "SERVICE-EXECUTOR spawn services"
        );
    }
}

impl<C: Collection> BuildGraph for ServiceExecutor<C> {
    fn build_graph() -> fdi::DependencyGraph {
        fdi::DependencyGraph::default()
            .with(Self::init.with_event_handler("start", Self::start.wrap_with_block_on()))
    }
}

impl<C: Collection> ServiceExecutorInterface<C> for ServiceExecutor<C> {
    type Provider = Provider;

    fn get_provider(&self) -> Self::Provider {
        Provider {
            collection: self.collection.clone(),
            ipc_dir: self.config.ipc_path.to_path_buf(),
        }
    }

    fn run_service(id: u32) {
        match id {
            #[cfg(feature = "services")]
            0 => {
                fleek_service_fetcher::main();
            },
            #[cfg(feature = "services")]
            1 => {
                fleek_service_js_poc::main();
            },
            #[cfg(feature = "services")]
            2 => {
                fleek_service_ai::main();
            },
            1001 => {
                crate::test_services::io_stress::main();
            },
            _ => error!("Service {id} not found."),
        }
    }
}

impl<C: Collection> ConfigConsumer for ServiceExecutor<C> {
    const KEY: &'static str = "service-executor";
    type Config = ServiceExecutorConfig;
}

impl ExecutorProviderInterface for Provider {
    /// Make a connection to the provided service.
    async fn connect(&self, service_id: ServiceId) -> Option<UnixStream> {
        let _ = self.collection.get(service_id)?;
        let path = self.ipc_dir.join(format!("service-{service_id}/conn"));

        trace!("called connect for {path:?}");
        match UnixStream::connect(path).await {
            Ok(s) => Some(s),
            Err(e) => {
                error!("failed to connect to service: {e}");
                None
            },
        }
    }
}
