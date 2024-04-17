use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use cid::multihash::{Code, MultihashDigest};
use cid::Cid;
use fleek_ipld::unixfs::Data;
use futures::TryStreamExt;
use hyper::client::{self, HttpConnector};
use hyper::{Body, Client, Request, Response, Uri};
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

pub struct IPFSOrigin<C: Collection> {
    client: Arc<Client<HttpsConnector<HttpConnector>, Body>>,
    gateways: Arc<Vec<Gateway>>,
    blockstore: C::BlockStoreInterface,
    gateway_timeout: Duration,
}

impl<C: Collection> Clone for IPFSOrigin<C> {
    fn clone(&self) -> Self {
        Self {
            client: self.client.clone(),
            gateways: self.gateways.clone(),
            blockstore: self.blockstore.clone(),
            gateway_timeout: self.gateway_timeout,
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
            gateway_timeout: config.gateway_timeout,
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
                                return Err(Error::Blockstore(format!("{e}")));
                            }
                        }

                        // TODO(matthias): is the data verification sufficient?
                        loop {
                            match car_reader.next_block().await {
                                Ok(Some((cid, data))) => {
                                    if nodes.contains(&cid) {
                                        verify_data(&cid, &data)?;
                                        let data = if cid.codec() == 0x55 {
                                            Some(data.into())
                                        } else {
                                            decoder::decode_block(cid, data)
                                                .map_err(|e| Error::CarReader(format!("{e}")))?
                                        };

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

            match self.fetch_from_gateway(req, gateway).await {
                Ok(hash) => return Ok(hash),
                Err(e) => match e {
                    Error::Blockstore(info) => {
                        error!("{info:?}. Stopping request.");
                        return Err(anyhow!("Request for CID {requested_cid} failed: {info:?}"));
                    },
                    Error::Request(info) => {
                        error!("{info:?}. Moving to next gateway.");
                    },
                    Error::Redirect(info) => {
                        error!("{info:?}. Moving to next gateway.");
                    },
                    Error::CarReader(info) => {
                        error!("{info:?}. Moving to next gateway.");
                    },
                },
            }
        }
        Err(anyhow!("Failed to fetch data from gateways."))
    }

    async fn fetch_from_gateway(
        &self,
        request: Request<Body>,
        gateway: &Gateway,
    ) -> Result<Blake3Hash, Error> {
        match timeout(self.gateway_timeout, self.client.request(request)).await {
            Ok(Ok(res)) => {
                match res.status().as_u16() {
                    200..=299 => {
                        // The gateway responded succesfully
                        self.stream_car_into_blockstore(res.into_body()).await
                    },
                    300..=399 => {
                        info!(
                            "Gateway {} returned redirect error code, trying one redirect",
                            gateway.authority
                        );
                        // This is the redirect code we should try to redirect one time to the
                        // proper location
                        self.handle_redirect(res).await
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

                        Err(Error::Redirect("Response was not successfull".into()))
                    },
                }
            },
            Ok(Err(e)) => Err(Error::Request(format!("Request failed: {e}"))),
            Err(_) => Err(Error::Request("Request timed out".into())),
        }
    }

    async fn handle_redirect(&self, response: Response<Body>) -> Result<Blake3Hash, Error> {
        let headers = response.headers();
        let location_header = headers
            .get("Location")
            .context("")
            .map_err(|_| Error::Redirect("Location header is missing".into()))?;
        let new_location = location_header
            .to_str()
            .map_err(|e| Error::Redirect(format!("Failed to parse header: {e}")))?;
        let new_uri = format!("{new_location}?format=car");
        let new_req = Request::builder()
            .uri(new_uri)
            .header("Accept", "application/vnd.ipld.car;version=1")
            .header("Connection", "keep-alive")
            .body(Body::default())
            .map_err(|e| Error::Redirect(format!("Failed to build request: {e}")))?;

        match timeout(self.gateway_timeout, self.client.request(new_req)).await {
            Ok(Ok(new_res)) => {
                let status = new_res.status();
                if status.is_success() {
                    self.stream_car_into_blockstore(new_res.into_body()).await
                } else {
                    Err(Error::Redirect("Response was not successful".into()))
                }
            },
            Ok(Err(e)) => Err(Error::Redirect(format!("Request failed: {e}"))),
            Err(_) => Err(Error::Redirect("Request timed out".into())),
        }
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
