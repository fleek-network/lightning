mod config;

use std::future::Future;
use std::marker::PhantomData;
use std::time::Duration;

use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::types::{Blake3Hash, CompressionAlgorithm};
use lightning_interfaces::{
    BlockStoreInterface,
    ConfigConsumer,
    IncrementalPutInterface,
    OriginFetcherInterface,
};
use reqwest::Client;

use crate::config::Config;

pub struct HttpOriginFetcher<C: Collection> {
    client: Client,
    blockstore: C::BlockStoreInterface,
    _marker: PhantomData<C>,
}

impl<C: Collection> Clone for HttpOriginFetcher<C> {
    fn clone(&self) -> Self {
        Self {
            client: self.client.clone(),
            blockstore: self.blockstore.clone(),
            _marker: PhantomData,
        }
    }
}

impl<C: Collection> ConfigConsumer for HttpOriginFetcher<C> {
    const KEY: &'static str = "http-origin-fetcher";
    type Config = Config;
}

impl<C: Collection> OriginFetcherInterface<C> for HttpOriginFetcher<C> {
    fn init(_: Self::Config, blockstore: C::BlockStoreInterface) -> anyhow::Result<Self> {
        let client = Client::new();
        Ok(Self {
            client,
            blockstore,
            _marker: PhantomData,
        })
    }

    async fn fetch(&self, address: Vec<u8>) -> anyhow::Result<Blake3Hash> {
        let resp = self
            .client
            .get(String::from_utf8(address)?)
            .timeout(Duration::from_millis(500))
            .send()
            .await?;
        let data = resp.bytes().await?;
        // Todo: do integrity check.
        let mut putter = self.blockstore.put(None);
        putter.write(data.as_ref(), CompressionAlgorithm::Uncompressed)?;
        putter.finalize().await.map_err(Into::into)
    }
}
