use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Context};
use cid::multihash::{Code, MultihashDigest};
use cid::Cid;
use hyper::client::{self, HttpConnector};
use hyper::{Body, Client, Request, Uri};
use hyper_rustls::{ConfigBuilderExt, HttpsConnector};
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::types::{Blake3Hash, CompressionAlgorithm};
use lightning_interfaces::{
    BlockStoreInterface,
    ConfigConsumer,
    IncrementalPutInterface,
    OriginFetcherInterface,
};
use tokio::io::AsyncReadExt;
use tokio::time::timeout;
use tracing::info;
pub mod config;
pub use config::Config;
use config::Gateway;
mod ipfs_stream;
pub use ipfs_stream::IPFSStream;
use tokio_util::io::StreamReader;
#[cfg(test)]
mod tests;

const GATEWAY_TIMEOUT: Duration = Duration::from_millis(500);

pub struct IPFSOrigin<C: Collection> {
    client: Arc<Client<HttpsConnector<HttpConnector>, Body>>,
    gateways: Vec<Gateway>,
    blockstore: C::BlockStoreInterface,
}

impl<C: Collection> ConfigConsumer for IPFSOrigin<C> {
    const KEY: &'static str = "origin-ipfs";

    type Config = Config;
}

impl<C: Collection> Clone for IPFSOrigin<C> {
    fn clone(&self) -> Self {
        todo!()
    }
}

impl<C: Collection> OriginFetcherInterface<C> for IPFSOrigin<C> {
    fn init(config: Config, blockstore: C::BlockStoreInterface) -> anyhow::Result<Self> {
        // Prepare the TLS client config
        let tls = rustls::ClientConfig::builder()
            .with_safe_defaults()
            .with_native_roots()
            .with_no_client_auth();

        // Prepare the HTTPS connector
        let https = hyper_rustls::HttpsConnectorBuilder::new()
            .with_tls_config(tls)
            .https_or_http()
            .enable_http1()
            .build();

        // Build the hyper client from the HTTPS connector.
        let client: Client<_, hyper::Body> = client::Client::builder().build(https);

        Ok(IPFSOrigin {
            client: Arc::new(client),
            gateways: config.gateways,
            blockstore,
        })
    }

    async fn fetch(&self, identifier: Vec<u8>) -> anyhow::Result<Blake3Hash> {
        let requested_cid =
            Cid::try_from(identifier).with_context(|| "Failed to parse uri into cid")?;

        let stream = self.fetch_from_gateway(&requested_cid).await?;

        let mut blockstore_putter = self.blockstore.put(None);
        let mut read = StreamReader::new(stream);
        let mut buf = [0; 1024];
        let mut bytes: Vec<u8> = Vec::new();
        let comp = CompressionAlgorithm::Uncompressed; // clippy

        loop {
            let len = read.read(&mut buf).await.unwrap();
            if let Err(e) = blockstore_putter.write(&buf[..len], comp) {
                anyhow::bail!("Failed to write to the blockstore: {e}");
            }
            bytes.extend(&buf[..len]);
            if len == 0 {
                break;
            }
        }

        let valid = match Code::try_from(requested_cid.hash().code()) {
            Ok(hasher) => &hasher.digest(&bytes) == requested_cid.hash(),
            _ => false,
        };
        if valid {
            blockstore_putter
                .finalize()
                .await
                .map_err(|e| anyhow!("failed to finalize in blockstore: {e}"))
        } else {
            Err(anyhow!("Data verification failed"))
        }
    }
}

impl<C: Collection> IPFSOrigin<C> {
    async fn fetch_from_gateway(&self, requested_cid: &Cid) -> anyhow::Result<IPFSStream> {
        for gateway in self.gateways.iter() {
            let url = Uri::builder()
                .scheme(gateway.protocol.as_str())
                .authority(gateway.authority.as_str())
                .path_and_query(format!("/ipfs/{requested_cid}"))
                .build()?;

            let req = Request::builder()
                .uri(url)
                .header("Accept", "application/vnd.ipld.raw")
                .header("Connection", "keep-alive")
                .body(Body::default())?;

            match timeout(GATEWAY_TIMEOUT, self.client.request(req)).await {
                Ok(Ok(res)) => {
                    let body = res.into_body();
                    return Ok(IPFSStream::new(*requested_cid, body));
                },
                Ok(Err(e)) => {
                    info!(
                        "Failed to fetch from gateway {gateway:?}, moving onto the next gateway: {e:?}"
                    );
                    continue;
                },
                Err(_) => {
                    info!(
                        "Timeout while fetching from gateway {gateway:?}, moving onto the next gateway"
                    );
                    continue;
                },
            }
        }
        Err(anyhow::anyhow!("No response from gateways."))
    }
}
