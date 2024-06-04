mod config;
#[cfg(test)]
mod tests;

use std::time::Duration;

use fast_sri::IntegrityMetadata;
use lightning_interfaces::prelude::*;
use lightning_interfaces::types::{Blake3Hash, CompressionAlgorithm};
use reqwest::{Client, ClientBuilder, Url};

pub use crate::config::Config;

pub struct HttpOrigin<C: Collection> {
    client: Client,
    blockstore: C::BlockstoreInterface,
}

impl<C: Collection> Clone for HttpOrigin<C> {
    fn clone(&self) -> Self {
        Self {
            client: self.client.clone(),
            blockstore: self.blockstore.clone(),
        }
    }
}

impl<C: Collection> HttpOrigin<C> {
    pub fn new(_: Config, blockstore: C::BlockstoreInterface) -> anyhow::Result<Self> {
        let client = ClientBuilder::new()
            .use_rustls_tls()
            .build()
            .expect("Unable to make reqwest https client in http origin");
        Ok(Self { client, blockstore })
    }

    pub async fn fetch(&self, uri: &[u8]) -> anyhow::Result<Blake3Hash> {
        let (url, sri) = get_url_and_sri(uri)?;
        let resp = self
            .client
            .get(url)
            .timeout(Duration::from_millis(1000))
            .send()
            .await?;
        let mut data: Vec<u8> = resp.bytes().await?.into();

        // We verify before inserting any blocks
        if let Some(integrity_metadata) = sri {
            let (is_valid, verified_data) = integrity_metadata.verify(data);
            if !is_valid {
                anyhow::bail!("sri failed: invalid digest");
            }
            data = verified_data;
        }

        let mut putter = self.blockstore.put(None);
        putter.write(data.as_ref(), CompressionAlgorithm::Uncompressed)?;
        putter.finalize().await.map_err(Into::into)
    }
}

pub(crate) fn get_url_and_sri(uri: &[u8]) -> anyhow::Result<(Url, Option<IntegrityMetadata>)> {
    let uri_str = String::from_utf8(uri.to_vec())?;
    let (url, sri) = uri_str
        .split_once("#integrity=")
        .map(|(url, hash)| (Url::parse(url), Some(hash)))
        .unwrap_or_else(|| (Url::parse(uri_str.as_str()), None));

    let integrity: Option<IntegrityMetadata> = if let Some(sri) = sri {
        Some(sri.parse()?)
    } else {
        None
    };

    Ok((url?, integrity))
}
