use std::marker::PhantomData;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

use fxhash::FxHashSet;
use infusion::c;
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::types::ServiceId;
use lightning_interfaces::{
    ApplicationInterface,
    BlockstoreInterface,
    ConfigConsumer,
    ExecutorProviderInterface,
    FetcherSocket,
    ServiceExecutorInterface,
    WithStartAndShutdown,
};
use resolved_pathbuf::ResolvedPathBuf;
use serde::{Deserialize, Serialize};
use tokio::net::UnixStream;
use tokio::sync::Notify;
use tracing::{error, trace};
use triomphe::Arc;

use crate::service::{spawn_service, Context, ServiceCollection};

pub struct ServiceExecutor<C: Collection> {
    config: ServiceExecutorConfig,
    is_running: Arc<AtomicBool>,
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
            ipc_path: "~/.lightning/ipc"
                .try_into()
                .expect("Failed to resolve path"),
        }
    }
}

impl ServiceExecutorConfig {
    pub fn test_default() -> Self {
        Self {
            services: Default::default(),
            ipc_path: "~/.lightning-test/ipc"
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

impl<C: Collection> ServiceExecutorInterface<C> for ServiceExecutor<C> {
    type Provider = Provider;

    fn init(
        config: Self::Config,
        blockstore: &C::BlockstoreInterface,
        fetcher_socket: FetcherSocket,
        query_runner: c!(C::ApplicationInterface::SyncExecutor),
    ) -> anyhow::Result<Self> {
        let ctx = Arc::new(Context {
            kill: Arc::new(Notify::new()),
            blockstore_path: blockstore.get_root_dir(),
            ipc_path: config.ipc_path.to_path_buf(),
            fetcher_socket,
            query_runner,
        });

        Ok(ServiceExecutor {
            config,
            is_running: Arc::new(AtomicBool::new(false)),
            collection: ServiceCollection::default(),
            ctx,
            p: PhantomData,
        })
    }

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
            _ => eprintln!("Service {id} not found."),
        }
    }
}

impl<C: Collection> WithStartAndShutdown for ServiceExecutor<C> {
    fn is_running(&self) -> bool {
        self.is_running.load(Ordering::Relaxed)
    }

    async fn start(&self) {
        self.is_running.store(true, Ordering::Relaxed);

        for &id in self.config.services.iter() {
            let handle = spawn_service(id, self.ctx.clone()).await;
            self.collection.insert(id, handle);
        }
    }

    async fn shutdown(&self) {
        self.is_running.store(false, Ordering::Relaxed);
        self.ctx.kill.notify_waiters();
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
