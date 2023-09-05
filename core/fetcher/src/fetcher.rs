use std::marker::PhantomData;

use anyhow::Result;
use async_trait::async_trait;
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::types::ImmutablePointer;
use lightning_interfaces::{Blake3Hash, ConfigConsumer, FetcherInterface, OriginProviderSocket};
use lightning_origin_ipfs::IPFSStream;

use crate::config::Config;

#[derive(Clone)]
pub struct Fetcher<C: Collection> {
    _origin: OriginProviderSocket<IPFSStream>,
    _collection: PhantomData<C>,
}

#[async_trait]
impl<C: Collection> FetcherInterface<C, IPFSStream> for Fetcher<C> {
    /// Initialize the fetcher.
    fn init(
        _config: Self::Config,
        _blockstore: C::BlockStoreInterface,
        _resolver: C::ResolverInterface,
        _origin: &C::OriginProviderInterface,
    ) -> anyhow::Result<Self> {
        todo!()
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
