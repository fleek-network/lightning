use std::marker::PhantomData;
use std::sync::atomic::{AtomicBool, Ordering};

use async_trait::async_trait;
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::{
    ConfigConsumer,
    ExecutorProviderInterface,
    ServiceExecutorInterface,
    WithStartAndShutdown,
};
use serde::{Deserialize, Serialize};
use triomphe::Arc;

use crate::collection::ServiceCollection;
use crate::deque::{CommandSender, CommandStealer};
use crate::handle::ServiceHandle;

pub struct ServiceExecutor<C: Collection> {
    is_running: Arc<AtomicBool>,
    collection: ServiceCollection,
    sender: CommandSender,
    stealer: CommandStealer,
    p: PhantomData<C>,
}

#[derive(Default, Serialize, Deserialize)]
pub struct ServiceExecutorConfig {}

#[derive(Clone)]
pub struct Provider {
    collection: ServiceCollection,
    stealer: CommandStealer,
}

impl<C: Collection> ServiceExecutorInterface<C> for ServiceExecutor<C> {
    type Provider = Provider;

    fn init(_config: Self::Config) -> anyhow::Result<Self> {
        let (sender, stealer) = crate::deque::chan();
        Ok(ServiceExecutor {
            is_running: Arc::new(AtomicBool::new(false)),
            collection: ServiceCollection::default(),
            sender,
            stealer,
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
        self.is_running.store(true, Ordering::Relaxed)
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
