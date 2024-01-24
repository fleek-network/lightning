use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Context};
use car_reader::{hyper_error, CarReader};
use cid::multihash::{Code, MultihashDigest};
use cid::Cid;
use futures::TryStreamExt;
use hyper::client::{self, HttpConnector};
use hyper::{Body, Client, Request, Uri};
use hyper_rustls::{ConfigBuilderExt, HttpsConnector};
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::types::{Blake3Hash, CompressionAlgorithm};
use lightning_interfaces::{BlockStoreInterface, IncrementalPutInterface};
use tokio::time::timeout;
use tracing::info;
pub mod config;
pub use config::Config;
use config::Gateway;
mod ipfs_stream;
pub use ipfs_stream::IPFSStream;
use tokio_util::io::StreamReader;

mod car_reader;
#[cfg(test)]
mod tests;

const GATEWAY_TIMEOUT: Duration = Duration::from_millis(500);

pub struct IPFSOrigin<C: Collection> {
    client: Arc<Client<HttpsConnector<HttpConnector>, Body>>,
    gateways: Arc<Vec<Gateway>>,
    blockstore: C::BlockStoreInterface,
}

impl<C: Collection> Clone for IPFSOrigin<C> {
    fn clone(&self) -> Self {
        Self {
            client: self.client.clone(),
            gateways: self.gateways.clone(),
            blockstore: self.blockstore.clone(),
        }
    }
}

impl<C: Collection> IPFSOrigin<C> {
    pub fn new(config: Config, blockstore: C::BlockStoreInterface) -> anyhow::Result<Self> {
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
            gateways: Arc::new(config.gateways),
            blockstore,
        })
    }

    pub async fn fetch(&self, uri: &[u8]) -> anyhow::Result<Blake3Hash> {
        let requested_cid = Cid::try_from(uri).with_context(|| "Failed to parse uri into cid")?;

        let body = self.fetch_from_gateway(&requested_cid).await?;
        let reader = StreamReader::new(body.map_err(hyper_error));
        let mut car_reader = CarReader::new(reader).await?;

        //let mut blockstore_putter = self.blockstore.put(None);
        //let mut buf = [0; 1024];
        //let mut bytes: Vec<u8> = Vec::new();
        //let comp = CompressionAlgorithm::Uncompressed; // clippy

        loop {
            match car_reader.next_block().await {
                Ok(Some((_cid, _data))) => {
                    // TODO(matthias): validation and further decoding
                    todo!()
                },
                Ok(None) => break,
                Err(e) => return Err(e),
            }
        }
        todo!()

        //loop {
        //    let len = read.read(&mut buf).await.unwrap();
        //    if let Err(e) = blockstore_putter.write(&buf[..len], comp) {
        //        anyhow::bail!("Failed to write to the blockstore: {e}");
        //    }
        //    bytes.extend(&buf[..len]);
        //    if len == 0 {
        //        break;
        //    }
        //}

        //let valid = match Code::try_from(requested_cid.hash().code()) {
        //    Ok(hasher) => &hasher.digest(&bytes) == requested_cid.hash(),
        //    _ => false,
        //};
        //if valid {
        //    blockstore_putter
        //        .finalize()
        //        .await
        //        .map_err(|e| anyhow!("failed to finalize in blockstore: {e}"))
        //} else {
        //    Err(anyhow!("Data verification failed"))
        //}
    }

    async fn fetch_from_gateway(&self, requested_cid: &Cid) -> anyhow::Result<Body> {
        for gateway in self.gateways.iter() {
            let url = Uri::builder()
                .scheme(gateway.protocol.as_str())
                .authority(gateway.authority.as_str())
                .path_and_query(format!("/ipfs/{requested_cid}"))
                .build()?;

            let req = Request::builder()
                .uri(url)
                //.header("Accept", "application/vnd.ipld.raw")
                //.header("Accept", "application/vnd.ipld.car")
                .header("Accept", "application/vnd.ipld.car;version=1")
                .header("Connection", "keep-alive")
                .body(Body::default())?;

            match timeout(GATEWAY_TIMEOUT, self.client.request(req)).await {
                Ok(Ok(res)) => {
                    return Ok(res.into_body());
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
