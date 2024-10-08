use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use b3fs::bucket::dir::writer::DirWriter;
use b3fs::bucket::file::writer::FileWriter;
use b3fs::entry::{BorrowedEntry, BorrowedLink};
use cid::Cid;
use fleek_ipld::decoder::fs::{DocId, IpldItem};
use fleek_ipld::decoder::reader::IpldReader;
use fleek_ipld::errors::IpldError;
use fleek_ipld::walker::downloader::{Downloader, Response};
use fleek_ipld::walker::stream::IpldStream;
use futures::TryStreamExt;
use hyper::client::{self, HttpConnector};
use hyper::{Body, Client, Request, Uri};
use hyper_rustls::{ConfigBuilderExt, HttpsConnector};
use lightning_interfaces::prelude::*;
use lightning_interfaces::types::Blake3Hash;
use tokio::time::timeout;
use tracing::info;

use crate::config::Gateway;
use crate::Config;

pub struct IPFSOrigin<C: NodeComponents> {
    client: Arc<Client<HttpsConnector<HttpConnector>, Body>>,
    gateways: Arc<Vec<Gateway>>,
    gateway_timeout: Duration,
    blockstore: C::BlockstoreInterface,
}

impl<C: NodeComponents> Clone for IPFSOrigin<C> {
    fn clone(&self) -> Self {
        Self {
            client: self.client.clone(),
            gateways: self.gateways.clone(),
            blockstore: self.blockstore.clone(),
            gateway_timeout: self.gateway_timeout,
        }
    }
}

impl<C: NodeComponents> Downloader for IPFSOrigin<C> {
    async fn download(&self, cid: &Cid) -> Result<Response, IpldError> {
        for gateway in self.gateways.iter() {
            let url: Uri = gateway.build_request(*cid).parse().map_err(|e| {
                IpldError::DownloaderError(format!("Failed to parse uri into cid: {e}"))
            })?;

            let req = Request::builder()
                .uri(url)
                .header("Connection", "keep-alive")
                .body(Body::default())
                .map_err(|e| IpldError::DownloaderError(format!("Failed to build request: {e}")))?;

            match timeout(self.gateway_timeout, self.client.request(req)).await {
                Ok(Ok(res)) => {
                    match res.status().as_u16() {
                        200..=299 => {
                            let body = res.into_body().map_err(|e| {
                                IpldError::DownloaderError(format!("Failed to get body: {e}"))
                            });
                            return Ok(Box::pin(body));
                        },
                        300..=399 => {
                            info!("Gateway {} returned redirect error code", gateway.authority);
                            // This is the redirect code we should try to redirect one time to the
                            // proper location
                            continue;
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
                    return Err(IpldError::DownloaderError(format!("Request failed: {e}")));
                },
                Err(_) => return Err(IpldError::DownloaderError("Request timed out".into())),
            }
        }
        Err(IpldError::DownloaderError(
            "Failed to fetch data from gateways.".into(),
        ))
    }
}

impl<C: NodeComponents> IPFSOrigin<C> {
    pub fn new(config: Config, blockstore: C::BlockstoreInterface) -> Result<Self> {
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

    pub async fn fetch(&self, uri: &[u8]) -> Result<Blake3Hash> {
        let requested_cid = Cid::try_from(uri).with_context(|| "Failed to parse uri into cid")?;
        let mut stream = IpldStream::builder()
            .reader(IpldReader::default())
            .downloader(self.clone())
            .build();

        stream.start(requested_cid).await;
        let mut hash: [u8; 32] = [0; 32];
        let bucket = self.blockstore.get_bucket();

        loop {
            let item = stream.next().await?;
            let mut last_dir: Option<DirWriter> = None;
            match item {
                Some(IpldItem::ChunkedFile(chunk)) => {
                    let doc_id = chunk.id().clone();
                    let mut stream_file = stream.new_chunk_file_streamer(chunk).await;
                    let mut file_writer = FileWriter::new(&bucket).await?;
                    while let Some(chunk) = stream_file.next_chunk().await? {
                        file_writer.write(chunk.data()).await?;
                    }
                    self.insert_file_into_dir(&mut last_dir, &mut hash, file_writer, &doc_id)
                        .await?;
                },
                Some(IpldItem::File(file)) => {
                    let doc_id = file.id().clone();
                    let mut file_writer = FileWriter::new(&bucket).await?;
                    file_writer.write(file.data()).await?;
                    self.insert_file_into_dir(&mut last_dir, &mut hash, file_writer, &doc_id)
                        .await?;
                },
                Some(IpldItem::Dir(dir)) => {
                    if last_dir.is_some() {
                        hash = last_dir.take().unwrap().commit().await?;
                    }
                    let dir_writer = DirWriter::new(&bucket, dir.links().len());
                    last_dir.replace(dir_writer);
                },
                Some(IpldItem::Chunk(_)) => {
                    return Err(anyhow!("Chunked data is not supported"));
                },
                None => break,
            }
        }

        if hash == [0; 32] {
            return Err(anyhow!("Cannot calculate hash"));
        }

        Ok(hash)
    }

    async fn insert_file_into_dir(
        &self,
        last_dir: &mut Option<DirWriter>,
        hash: &mut [u8; 32],
        file_writer: FileWriter,
        doc_id: &DocId,
    ) -> Result<()> {
        let file_name = doc_id.file_name().unwrap_or_default();
        let temp_hash = file_writer.commit().await?;
        if let Some(dir) = last_dir.as_mut() {
            let entry = BorrowedEntry {
                name: file_name.as_bytes(),
                link: BorrowedLink::Content(&temp_hash),
            };
            dir.insert(entry).await?;
        }
        *hash = temp_hash;
        Ok(())
    }
}
