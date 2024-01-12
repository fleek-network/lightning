mod config;
#[cfg(test)]
mod tests;

use std::time::Duration;

use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::types::{Blake3Hash, CompressionAlgorithm};
use lightning_interfaces::{
    BlockStoreInterface,
    ConfigConsumer,
    IncrementalPutInterface,
    OriginFetcherInterface,
};
use reqwest::{Client, Url};

pub use crate::config::Config;

pub struct HttpOriginFetcher<C: Collection> {
    client: Client,
    blockstore: C::BlockStoreInterface,
}

impl<C: Collection> Clone for HttpOriginFetcher<C> {
    fn clone(&self) -> Self {
        Self {
            client: self.client.clone(),
            blockstore: self.blockstore.clone(),
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
        Ok(Self { client, blockstore })
    }

    async fn fetch(&self, uri: &[u8]) -> anyhow::Result<Blake3Hash> {
        let (url, _hash) = get_url_and_sri(uri)?;
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

pub(crate) fn get_url_and_sri(uri: &[u8]) -> anyhow::Result<(Url, Option<String>)> {
    let uri_str = String::from_utf8(uri.to_vec())?;
    let (url, hash) = uri_str
        .split_once("#integrity=")
        .map(|(url, hash)| (Url::parse(url), Some(hash.to_string())))
        .unwrap_or_else(|| (Url::parse(uri_str.as_str()), None));

    if hash.as_ref().map(|value| value.is_empty()).unwrap_or(false) {
        anyhow::bail!("invalid integrity value");
    }

    Ok((url?, hash))
}
