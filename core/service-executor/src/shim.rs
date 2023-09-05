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
    ServiceHandleInterface,
    WithStartAndShutdown,
};
use resolved_pathbuf::ResolvedPathBuf;
use serde::{Deserialize, Serialize};
use triomphe::Arc;

use crate::callback::make_callback;
use crate::collection::ServiceCollection;
use crate::deque::{CommandSender, CommandStealer};
use crate::handle;
use crate::handle::ServiceHandle;

pub struct ServiceExecutor<C: Collection> {
    config: ServiceExecutorConfig,
    is_running: Arc<AtomicBool>,
    collection: ServiceCollection,
    sender: CommandSender,
    stealer: CommandStealer,
    blockstore: ResolvedPathBuf,
    p: PhantomData<C>,
}

#[derive(Default, Serialize, Deserialize)]
pub struct ServiceExecutorConfig {
    services: FxHashSet<ServiceId>,
}

#[derive(Clone)]
pub struct Provider {
    collection: ServiceCollection,
    stealer: CommandStealer,
}

impl<C: Collection> ServiceExecutorInterface<C> for ServiceExecutor<C> {
    type Provider = Provider;

    fn init(config: Self::Config, blockstore: &C::BlockStoreInterface) -> anyhow::Result<Self> {
        let (sender, stealer) = crate::deque::chan();
        Ok(ServiceExecutor {
            config,
            is_running: Arc::new(AtomicBool::new(false)),
            collection: ServiceCollection::default(),
            sender,
            stealer,
            blockstore: blockstore.get_root_dir().try_into()?,
            p: PhantomData,
        })
    }

    fn get_provider(&self) -> Self::Provider {
        Provider {
            collection: self.collection.clone(),
            stealer: self.stealer.clone(),
        }
    }
}

#[async_trait]
impl<C: Collection> WithStartAndShutdown for ServiceExecutor<C> {
    fn is_running(&self) -> bool {
        self.is_running.load(Ordering::Relaxed)
    }

    async fn start(&self) {
        self.is_running.store(true, Ordering::Relaxed);

        if !self.config.services.is_empty() {
            let request_sender = make_callback(self.sender.clone(), fn_sdk::api::on_event_response);

            fn_sdk::api::setup(fn_sdk::internal::OnStartArgs {
                request_sender,
                block_store_path: self.blockstore.clone(),
            });
        }

        for handle in get_all_services() {
            let id = handle.get_service_id();
            if self.config.services.contains(&id) {
                log::info!("Enabling service {id}");
                self.collection.insert(handle);
            }
        }
    }

    async fn shutdown(&self) {
        self.is_running.store(false, Ordering::Relaxed)
    }
}

impl<C: Collection> ConfigConsumer for ServiceExecutor<C> {
    const KEY: &'static str = "service-executor";
    type Config = ServiceExecutorConfig;
}

impl ExecutorProviderInterface for Provider {
    type Handle = ServiceHandle;
    type Stealer = CommandStealer;

    #[inline(always)]
    fn get_work_stealer(&self) -> Self::Stealer {
        self.stealer.clone()
    }

    #[inline(always)]
    fn get_service_handle(
        &self,
        service_id: lightning_interfaces::types::ServiceId,
    ) -> Option<Self::Handle> {
        self.collection.get_handle(service_id)
    }
}

fn get_all_services() -> Vec<ServiceHandle> {
    vec![
        handle!(0, fleek_service_ping_example),
        handle!(1, fleek_service_big_buck_bunny),
    ]
}
