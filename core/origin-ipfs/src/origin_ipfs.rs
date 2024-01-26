use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use cid::multihash::{Code, MultihashDigest};
use cid::Cid;
use fleek_ipld::unixfs::Data;
use futures::TryStreamExt;
use hyper::client::{self, HttpConnector};
use hyper::{Body, Client, Request, Uri};
use hyper_rustls::{ConfigBuilderExt, HttpsConnector};
use libipld::pb::PbNode;
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::types::{Blake3Hash, CompressionAlgorithm};
use lightning_interfaces::{BlockStoreInterface, IncrementalPutInterface};
use tokio::time::timeout;
use tokio_util::io::StreamReader;
use tracing::info;

use crate::car_reader::{hyper_error, CarReader};
use crate::config::Gateway;
use crate::{decoder, Config};

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
    pub fn new(config: Config, blockstore: C::BlockStoreInterface) -> Result<Self> {
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

    pub async fn fetch(&self, uri: &[u8]) -> Result<Blake3Hash> {
        // Disclaimer(matthias): this method is unpolished and will be improved in due time
        let requested_cid = Cid::try_from(uri).with_context(|| "Failed to parse uri into cid")?;

        let body = self.fetch_from_gateway(&requested_cid).await?;
        let reader = StreamReader::new(body.map_err(hyper_error));
        let mut car_reader = CarReader::new(reader).await?;

        let mut blockstore_putter = self.blockstore.put(None);
        let comp = CompressionAlgorithm::Uncompressed; // clippy

        // TODO(matthias): we assume that the merkle dag is flat for now,
        // but we have to support general merke dags in the future
        match car_reader.next_block().await {
            Ok(Some((cid, data))) => {
                verify_data(&cid, &data)?;
                match cid.codec() {
                    0x55 => {
                        // TODO(matthias): make sure that if the codec of the root block is raw
                        // (0x55), then there will never be any more blocks after that
                        if let Err(e) = blockstore_putter.write(&data, comp) {
                            return Err(anyhow!("Failed to write to the blockstore: {e}"));
                        }
                    },
                    0x70 => {
                        let node = PbNode::from_bytes(data.into())?;
                        let mut nodes = HashSet::new();
                        for links in &node.links {
                            nodes.insert(links.cid);
                        }

                        if let Some(data) = node.data {
                            let data = match Data::try_from(data.as_ref()) {
                                Ok(unixfs) => unixfs.Data.to_vec().into(),
                                Err(_) => data,
                            };
                            if let Err(e) = blockstore_putter.write(&data, comp) {
                                return Err(anyhow!("Failed to write to the blockstore: {e}"));
                            }
                        }

                        // TODO(matthias): is the data verification sufficient?
                        loop {
                            match car_reader.next_block().await {
                                Ok(Some((cid, data))) => {
                                    if nodes.contains(&cid) {
                                        verify_data(&cid, &data)?;
                                        let data = decoder::decode_block(cid, data)?;

                                        if let Some(data) = data {
                                            if let Err(e) = blockstore_putter.write(&data, comp) {
                                                return Err(anyhow!(
                                                    "Failed to write to the blockstore: {e}"
                                                ));
                                            }
                                        }
                                    }
                                },
                                Ok(None) => break,
                                Err(e) => return Err(e),
                            }
                        }
                    },
                    codec => return Err(anyhow!("Unsupported codec found in CID: {codec}")),
                }
            },
            Ok(None) => {
                // TODO(matthias): what if there is no block?
                return Err(anyhow!("The car file was empty"));
            },
            Err(e) => return Err(e),
        }

        blockstore_putter
            .finalize()
            .await
            .map_err(|e| anyhow!("Failed to finalize blockstore put: {e}"))
    }

    async fn fetch_from_gateway(&self, requested_cid: &Cid) -> Result<Body> {
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

fn verify_data(cid: &Cid, data: &[u8]) -> Result<()> {
    let valid = match Code::try_from(cid.hash().code()) {
        Ok(hasher) => &hasher.digest(data) == cid.hash(),
        _ => false,
    };
    if valid {
        Ok(())
    } else {
        Err(anyhow!("Data verification failed for CID: {cid}"))
    }
}
