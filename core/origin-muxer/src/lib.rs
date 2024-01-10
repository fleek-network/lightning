mod config;

use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::Arc;
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::{ConfigConsumer, OriginProviderInterface, OriginProviderSocket, WithStartAndShutdown};
use crate::config::Config;

pub enum OriginType {
    Http,
    Ipfs,
}

pub struct OriginMuxer<C: Collection> {
    origins: HashMap<OriginType, Arc<C::OriginFetcherInterface>>,
    _marker: PhantomData<C>,
}

impl<C: Collection> ConfigConsumer for OriginMuxer<C> {
    const KEY: &'static str = "origin-muxer";
    type Config = Config;
}

impl<C: Collection> WithStartAndShutdown for OriginMuxer<C> {
    fn is_running(&self) -> bool {
        todo!()
    }

    async fn start(&self) {
        todo!()
    }

    async fn shutdown(&self) {
        todo!()
    }
}

impl<C: Collection> OriginProviderInterface<C> for OriginMuxer<C> {
    fn init(config: Self::Config, blockstore: C::BlockStoreInterface) -> anyhow::Result<Self> {
        todo!()
    }

    fn get_socket(&self) -> OriginProviderSocket {
        todo!()
    }
}