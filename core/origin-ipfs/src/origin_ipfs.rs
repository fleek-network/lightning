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
use tracing::{error, info};

use crate::car_reader::{hyper_error, CarReader};
use crate::config::Gateway;
use crate::error::Error;
use crate::{decoder, Config};

const GATEWAY_TIMEOUT: Duration = Duration::from_millis(3000);

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

    pub async fn stream_car_into_blockstore(
        &self,
        response_body: Body,
    ) -> Result<Blake3Hash, Error> {
        // Disclaimer(matthias): this method is unpolished and will be improved in due time
        let reader = StreamReader::new(response_body.map_err(hyper_error));
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
                            return Err(Error::Blockstore(format!("{e}")));
                        }
                    },
                    0x70 => {
                        let node = PbNode::from_bytes(data.into())
                            .map_err(|e| Error::CarReader(format!("{e}")))?;
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
                                // TODO: In this case we probably don't want to move in to the next
                                // gateway
                                return Err(Error::Blockstore(format!("{e}")));
                            }
                        }

                        // TODO(matthias): is the data verification sufficient?
                        loop {
                            match car_reader.next_block().await {
                                Ok(Some((cid, data))) => {
                                    if nodes.contains(&cid) {
                                        verify_data(&cid, &data)?;
                                        let data = decoder::decode_block(cid, data)
                                            .map_err(|e| Error::CarReader(format!("{e}")))?;

                                        if let Some(data) = data {
                                            if let Err(e) = blockstore_putter.write(&data, comp) {
                                                return Err(Error::Blockstore(format!("{e}")));
                                            }
                                        }
                                    }
                                },
                                Ok(None) => break,
                                Err(e) => return Err(e),
                            }
                        }
                    },
                    codec => {
                        return Err(Error::CarReader(format!(
                            "Unsupported codec found in CID: {codec}"
                        )));
                    },
                }
            },
            Ok(None) => {
                // TODO(matthias): what if there is no block?
                return Err(Error::CarReader("The car file was empty".into()));
            },
            Err(e) => return Err(e),
        }

        // TODO: If finalizing the put fails, we probably don't need to move to the next gateway
        blockstore_putter
            .finalize()
            .await
            .map_err(|e| Error::Blockstore(format!("{e}")))
    }

    pub async fn fetch(&self, uri: &[u8]) -> Result<Blake3Hash> {
        let requested_cid = Cid::try_from(uri).with_context(|| "Failed to parse uri into cid")?;
        for gateway in self.gateways.iter() {
            let url = Uri::builder()
                .scheme(gateway.protocol.as_str())
                .authority(gateway.authority.as_str())
                .path_and_query(format!("/ipfs/{requested_cid}"))
                .build()?;

            let req = Request::builder()
                .uri(url)
                .header("Accept", "application/vnd.ipld.car;version=1")
                .header("Connection", "keep-alive")
                .body(Body::default())?;

            // TODO: simplify these matches
            match timeout(GATEWAY_TIMEOUT, self.client.request(req)).await {
                Ok(Ok(res)) => {
                    match res.status().as_u16() {
                        200..=299 => {
                            // The gateway responded succesfully
                            match self.stream_car_into_blockstore(res.into_body()).await {
                                Ok(hash) => return Ok(hash),
                                Err(e) => match e {
                                    Error::Blockstore(e) => return Err(anyhow!("{e}")),
                                    Error::CarReader(e) => {
                                        error!(
                                            "Failed to parse car file from gateway {}: {e}",
                                            gateway.authority
                                        );
                                        continue;
                                    },
                                },
                            }
                        },
                        300..=399 => {
                            info!(
                                "Gateway {} returned redirect error code trying one redirect",
                                gateway.authority
                            );
                            // This is the redirect code we should try to redirect one time to the
                            // proper location
                            let headers = res.headers();
                            let Some(location_header) = headers.get("Location") else {
                                info!(
                                    "Gateway {} redirected but the location header is missing",
                                    gateway.authority
                                );
                                continue;
                            };
                            let Ok(new_location) = location_header.to_str() else {
                                info!(
                                    "Gateway {} redirected but the location header non-ASCII characters, moving to next gateway",
                                    gateway.authority
                                );
                                continue;
                            };
                            let new_uri = format!("{new_location}?format=car");
                            let Ok(new_req) = Request::builder()
                                .uri(new_uri)
                                .header("Accept", "application/vnd.ipld.car;version=1")
                                .header("Connection", "keep-alive")
                                .body(Body::default())
                            else {
                                info!(
                                    "Gateway {} redirected but building the new request failed, moving to next gateway",
                                    gateway.authority
                                );
                                continue;
                            };

                            match timeout(GATEWAY_TIMEOUT, self.client.request(new_req)).await {
                                Ok(Ok(new_res)) => {
                                    let status = new_res.status();
                                    if status.is_success() {
                                        match self.stream_car_into_blockstore(res.into_body()).await
                                        {
                                            Ok(hash) => return Ok(hash),
                                            Err(e) => match e {
                                                Error::Blockstore(e) => return Err(anyhow!("{e}")),
                                                Error::CarReader(e) => {
                                                    error!(
                                                        "Failed to parse car file from gateway {}: {e}",
                                                        gateway.authority
                                                    );
                                                    continue;
                                                },
                                            },
                                        }
                                    } else {
                                        info!(
                                            "Gateway {} response was not successful after redirect, moving to next gateway",
                                            gateway.authority
                                        );
                                        continue;
                                    }
                                },
                                Ok(Err(e)) => {
                                    info!(
                                        "Failed to fetch from gateway {}, moving onto the next gateway: {e:?}",
                                        gateway.authority
                                    );
                                    continue;
                                },
                                Err(_) => {
                                    info!(
                                        "Redirect from gateway {} timed out, moving to next gateway",
                                        gateway.authority
                                    );
                                    continue;
                                },
                            }
                        },
                        _ => {
                            // This is either informational(100-199), error(300-399, server
                            // error(400-499) so lets try another gateway
                            // todo(dalton): We should look into what could cause informational
                            // 100-199 and see if there is anything we can do here
                            info!(
                                "Gateway {} response was not successful, moving on to the next gateway",
                                gateway.authority
                            );
                            continue;
                        },
                    }
                },
                Ok(Err(e)) => {
                    info!(
                        "Failed to fetch from gateway {}, moving onto the next gateway: {e:?}",
                        gateway.authority
                    );
                    continue;
                },
                Err(_) => {
                    info!(
                        "Timeout while fetching from gateway {}, moving onto the next gateway",
                        gateway.authority
                    );
                    continue;
                },
            }
        }
        Err(anyhow!("No response from gateways."))
    }
}

fn verify_data(cid: &Cid, data: &[u8]) -> Result<(), Error> {
    let valid = match Code::try_from(cid.hash().code()) {
        Ok(hasher) => &hasher.digest(data) == cid.hash(),
        _ => false,
    };
    if valid {
        Ok(())
    } else {
        Err(Error::CarReader(format!(
            "Data verification failed for CID: {cid}"
        )))
    }
}
