use std::marker::PhantomData;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::types::{ImmutablePointer, OriginProvider};
use lightning_interfaces::{
    Blake3Hash,
    BlockStoreInterface,
    ConfigConsumer,
    FetcherInterface,
    OriginProviderInterface,
    OriginProviderSocket,
    ResolverInterface,
};

use crate::config::Config;

#[derive(Clone)]
pub struct Fetcher<C: Collection> {
    origin_socket: OriginProviderSocket,
    blockstore: C::BlockStoreInterface,
    resolver: C::ResolverInterface,
    _collection: PhantomData<C>,
}

#[async_trait]
impl<C: Collection> FetcherInterface<C> for Fetcher<C> {
    /// Initialize the fetcher.
    fn init(
        _config: Self::Config,
        blockstore: C::BlockStoreInterface,
        resolver: C::ResolverInterface,
        origin: &C::OriginProviderInterface,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            origin_socket: origin.get_socket(),
            blockstore,
            resolver,
            _collection: PhantomData,
        })
    }

    /// Fetches the data from the corresponding origin, puts it in the blockstore, and stores the
    /// mapping using the resolver. If the mapping already exists, the data will not be fetched
    /// from origin again.
    async fn put(&self, pointer: ImmutablePointer) -> Result<Blake3Hash> {
        match pointer.origin {
            OriginProvider::IPFS => {
                if let Some(resolved_pointer) = self.resolver.get_blake3_hash(pointer.clone()).await
                {
                    Ok(resolved_pointer.hash)
                } else {
                    let Ok(Ok(hash)) = self.origin_socket.run(pointer.uri.clone()).await else {
                        return Err(anyhow!("Failed to get response"));
                    };
                    // TODO(matthias): use different hasing algos
                    self.resolver.publish(hash, &[pointer]).await;
                    Ok(hash)
                }
            },
            _ => unreachable!(),
        }
    }

    /// Fetches the data from the blockstore. If the data does not exist in the blockstore, it will
    /// be fetched from the origin.
    async fn fetch(&self, hash: Blake3Hash) -> Result<()> {
        if self.blockstore.get_tree(&hash).await.is_some() {
            return Ok(());
        }
        if let Some(pointers) = self.resolver.get_origins(hash) {
            for resolved_pointer in pointers {
                if let Ok(res_hash) = self.put(resolved_pointer.pointer).await {
                    if res_hash == hash && self.blockstore.get_tree(&hash).await.is_some() {
                        return Ok(());
                    }
                }
            }
        }

        Err(anyhow!("Failed to fetch data"))
    }
}

impl<C: Collection> ConfigConsumer for Fetcher<C> {
    const KEY: &'static str = "fetcher";

    type Config = Config;
}
