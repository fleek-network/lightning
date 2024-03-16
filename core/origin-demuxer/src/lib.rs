mod config;
mod demuxer;
#[cfg(test)]
mod tests;

use std::marker::PhantomData;

use affair::{Executor, TokioSpawn};
use demuxer::Demuxer;
use lightning_interfaces::fdi::{BuildGraph, DependencyGraph};
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::{
    ConfigConsumer,
    ConfigProviderInterface,
    OriginProviderInterface,
    OriginProviderSocket,
};

pub use crate::config::Config;

pub struct OriginDemuxer<C: Collection> {
    socket: OriginProviderSocket,
    _collection: PhantomData<C>,
}

impl<C: Collection> ConfigConsumer for OriginDemuxer<C> {
    const KEY: &'static str = "origin-demuxer";
    type Config = Config;
}

impl<C: Collection> OriginDemuxer<C> {
    pub fn new(
        config: &C::ConfigProviderInterface,
        blockstore: &C::BlockstoreInterface,
    ) -> anyhow::Result<Self> {
        let config = config.get::<Self>();
        let demuxer = Demuxer::<C>::new(config, blockstore.clone())?;
        let socket = TokioSpawn::spawn_async_unordered(demuxer);
        Ok(Self {
            socket,
            _collection: PhantomData,
        })
    }
}

impl<C: Collection> OriginProviderInterface<C> for OriginDemuxer<C> {
    fn get_socket(&self) -> OriginProviderSocket {
        self.socket.clone()
    }
}

impl<C: Collection> BuildGraph for OriginDemuxer<C> {
    fn build_graph() -> DependencyGraph {
        DependencyGraph::default().with(Self::new)
    }
}
