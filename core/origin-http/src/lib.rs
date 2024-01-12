mod config;
#[cfg(test)]
mod tests;

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

pub use crate::config::Config;

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

    async fn fetch(&self, uri: &[u8]) -> anyhow::Result<Blake3Hash> {
        // Todo: work with slice instead of allocating a vec.
        let identifier = String::from_utf8(uri.to_vec())?;
        let (url, _hash) = if identifier.contains("#integrity=") {
            identifier
                .split_once("#integrity=")
                .map(|(url, hash)| (url, Some(hash)))
                .ok_or(anyhow::anyhow!("invalid identifier"))?
        } else {
            (identifier.as_str(), None)
        };

        if url.is_empty() {
            anyhow::bail!("invalid url");
        }

        // Todo: check that it's a valid URL.

        let resp = self
            .client
            .get(url)
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
