use std::marker::PhantomData;
use std::sync::atomic::{AtomicBool, Ordering};

use async_trait::async_trait;
use fxhash::FxHashSet;
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::types::ServiceId;
use lightning_interfaces::{
    BlockStoreInterface,
    ConfigConsumer,
    ExecutorProviderInterface,
    ServiceExecutorInterface,
    WithStartAndShutdown,
};
use resolved_pathbuf::ResolvedPathBuf;
use serde::{Deserialize, Serialize};
use tokio::net::UnixStream;
use triomphe::Arc;

use crate::service::ServiceCollection;

pub struct ServiceExecutor<C: Collection> {
    config: ServiceExecutorConfig,
    is_running: Arc<AtomicBool>,
    collection: ServiceCollection,
    blockstore: ResolvedPathBuf,
    p: PhantomData<C>,
}

#[derive(Serialize, Deserialize)]
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

#[derive(Clone)]
pub struct Provider {
    collection: ServiceCollection,
}

impl<C: Collection> ServiceExecutorInterface<C> for ServiceExecutor<C> {
    type Provider = Provider;

    fn init(config: Self::Config, blockstore: &C::BlockStoreInterface) -> anyhow::Result<Self> {
        Ok(ServiceExecutor {
            config,
            is_running: Arc::new(AtomicBool::new(false)),
            collection: ServiceCollection::default(),
            blockstore: blockstore.get_root_dir().try_into()?,
            p: PhantomData,
        })
    }

    fn get_provider(&self) -> Self::Provider {
        Provider {
            collection: self.collection.clone(),
        }
    }

    fn run_service(
        name: String,
        blockstore_path: std::path::PathBuf,
        ipc_socket: std::path::PathBuf,
    ) {
        todo!()
    }
}

#[async_trait]
impl<C: Collection> WithStartAndShutdown for ServiceExecutor<C> {
    fn is_running(&self) -> bool {
        self.is_running.load(Ordering::Relaxed)
    }

    async fn start(&self) {
        self.is_running.store(true, Ordering::Relaxed);

        // for handle in get_all_services() {
        //     let id = handle.get_service_id();
        //     if self.config.services.contains(&id) {
        //         info!("Enabling service {id}");
        //         self.collection.insert(handle);
        //     }
        // }
    }

    async fn shutdown(&self) {
        self.is_running.store(false, Ordering::Relaxed)
    }
}

impl<C: Collection> ConfigConsumer for ServiceExecutor<C> {
    const KEY: &'static str = "service-executor";
    type Config = ServiceExecutorConfig;
}

#[async_trait]
impl ExecutorProviderInterface for Provider {
    /// Make a connection to the provided service.
    async fn connect(&self, service_id: ServiceId) -> Option<UnixStream> {
        todo!()
    }
}
