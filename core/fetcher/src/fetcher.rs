use std::marker::PhantomData;

use anyhow::Result;
use async_trait::async_trait;
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::types::ImmutablePointer;
use lightning_interfaces::{
    Blake3Hash,
    ConfigConsumer,
    FetcherInterface,
    OriginProviderInterface,
    OriginProviderSocket,
};

use crate::config::Config;

pub struct Fetcher<C: Collection> {
    origin_socket: OriginProviderSocket,
    _collection: PhantomData<C>,
}

impl<C: Collection> Clone for Fetcher<C> {
    fn clone(&self) -> Self {
        Self {
            origin_socket: self.origin_socket.clone(),
            _collection: PhantomData,
        }
    }
}

#[async_trait]
impl<C: Collection> FetcherInterface<C> for Fetcher<C> {
    /// Initialize the fetcher.
    fn init(
        _config: Self::Config,
        _blockstore: C::BlockStoreInterface,
        _resolver: C::ResolverInterface,
        origin: &C::OriginProviderInterface,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            origin_socket: origin.get_socket(),
            _collection: PhantomData,
        })
    }

    /// Fetches the data from the corresponding origin, puts it in the blockstore, and stores the
    /// mapping using the resolver. If the mapping already exists, the data will not be fetched
    /// from origin again.
    async fn put(&self, _pointer: ImmutablePointer) -> Blake3Hash {
        todo!()
    }

    /// Fetches the data from the blockstore. If the data does not exist in the blockstore, it will
    /// be fetched from the origin.
    async fn fetch(&self, _hash: Blake3Hash) -> Result<()> {
        todo!()
    }
}

impl<C: Collection> ConfigConsumer for Fetcher<C> {
    const KEY: &'static str = "fetcher";

    type Config = Config;
}
