mod config;
mod demuxer;
#[cfg(test)]
mod tests;

use std::marker::PhantomData;

use affair::AsyncWorkerUnordered;
use demuxer::Demuxer;
use lightning_interfaces::prelude::*;
use lightning_interfaces::spawn_worker;

pub use crate::config::Config;

pub struct OriginDemuxer<C: NodeComponents> {
    socket: OriginProviderSocket,
    _components: PhantomData<C>,
}

impl<C: NodeComponents> ConfigConsumer for OriginDemuxer<C> {
    const KEY: &'static str = "origin-demuxer";
    type Config = Config;
}

impl<C: NodeComponents> OriginDemuxer<C> {
    pub fn new(
        config: &C::ConfigProviderInterface,
        blockstore: &C::BlockstoreInterface,
        fdi::Cloned(waiter): fdi::Cloned<ShutdownWaiter>,
    ) -> anyhow::Result<Self> {
        let config = config.get::<Self>();
        let demuxer = Demuxer::<C>::new(config, blockstore.clone())?;
        let socket = spawn_worker!(demuxer, "ORIGIN-DEMUXER", waiter, crucial);
        Ok(Self {
            socket,
            _components: PhantomData,
        })
    }
}

impl<C: NodeComponents> OriginProviderInterface<C> for OriginDemuxer<C> {
    fn get_socket(&self) -> OriginProviderSocket {
        self.socket.clone()
    }
}

impl<C: NodeComponents> BuildGraph for OriginDemuxer<C> {
    fn build_graph() -> fdi::DependencyGraph {
        fdi::DependencyGraph::default().with(Self::new)
    }
}
