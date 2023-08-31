use std::marker::PhantomData;

use async_trait::async_trait;
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::{
    ConfigConsumer,
    ExecutorProviderInterface,
    ServiceExecutorInterface,
    WithStartAndShutdown,
};
use serde::{Deserialize, Serialize};

use crate::collection::ServiceCollection;
use crate::deque::{CommandSender, CommandStealer};
use crate::handle::ServiceHandle;

pub struct ServiceExecutor<C: Collection> {
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
    /// Returns true if this system is running or not.
    fn is_running(&self) -> bool {
        true
    }

    /// Start the system, should not do anything if the system is already
    /// started.
    async fn start(&self) {}

    /// Send the shutdown signal to the system.
    async fn shutdown(&self) {}
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
