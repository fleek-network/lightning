mod config;
mod demuxer;
#[cfg(test)]
mod tests;

use std::marker::PhantomData;

use affair::AsyncWorkerUnordered;
use demuxer::Demuxer;
use lightning_interfaces::prelude::*;

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
        let socket = demuxer.spawn();
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
    fn build_graph() -> fdi::DependencyGraph {
        fdi::DependencyGraph::default().with(Self::new)
    }
}
