use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
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
use lightning_interfaces::{DirTrustedWriter, FileTrustedWriter};
use tokio::time::timeout;
use tracing::{debug, info};

use crate::config::Gateway;
use crate::Config;

/// { docid: (writer, parent) }
type DirStack<C> = HashMap<DocId, (c!(C::BlockstoreInterface::DirWriter), Option<DocId>)>;

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

        let mut dir_stack = DirStack::<C>::new();
        let mut hash = [0; 32];
        let mut dirs_to_commit = Vec::new();

        loop {
            let item = stream.next().await?;

            match item {
                Some((IpldItem::ChunkedFile(chunk), parent)) => {
                    let doc_id = chunk.id().clone();
                    let mut stream_file = stream.new_chunk_file_streamer(chunk).await;
                    let mut file_writer = self.blockstore.file_writer().await?;
                    while let Some(chunk) = stream_file.next_chunk().await? {
                        file_writer.write(chunk.data(), false).await?;
                    }
                    self.insert_file_into_dir(
                        &mut dir_stack,
                        parent,
                        file_writer,
                        &doc_id,
                        &mut dirs_to_commit,
                    )
                    .await?;
                },
                Some((IpldItem::File(file), parent)) => {
                    let doc_id = file.id().clone();
                    let mut file_writer = self.blockstore.file_writer().await?;
                    file_writer.write(file.data(), false).await?;
                    // store the hash in case this is a single file download
                    hash = self
                        .insert_file_into_dir(
                            &mut dir_stack,
                            parent,
                            file_writer,
                            &doc_id,
                            &mut dirs_to_commit,
                        )
                        .await?;
                },
                Some((IpldItem::Dir(dir), parent)) => {
                    // create a writer and insert it into the stack
                    let dir_writer = self.blockstore.dir_writer(dir.links().len()).await?;
                    dir_stack.insert(dir.id().clone(), (dir_writer, parent));
                },
                Some((IpldItem::Chunk(_), _)) => {
                    return Err(anyhow!("Chunked data is not supported"));
                },
                None => {
                    if !dir_stack.is_empty() {
                        return Err(anyhow!("incomplete stream"));
                    }

                    break;
                },
            }

            // finalize any complete dirs
            for id in std::mem::take(&mut dirs_to_commit) {
                let (writer, parent) = dir_stack.remove(&id).unwrap();
                let dir_hash = writer.commit().await?;

                // if we have a parent, insert it
                if let Some(parent) = parent {
                    if let Some((parent_writer, _)) = dir_stack.get_mut(&parent) {
                        let file_name = id.file_name().unwrap_or_default();
                        let entry = b3fs::entry::BorrowedEntry {
                            name: file_name.as_bytes(),
                            link: BorrowedLink::Content(&dir_hash),
                        };
                        parent_writer.insert(entry, false).await?;
                        if parent_writer.ready_to_commit() {
                            // will be resolved in the next pass
                            dirs_to_commit.push(parent);
                        }
                    }
                } else {
                    // the item is the root, store the hash
                    hash = dir_hash;
                }
            }
        }

        if hash == [0; 32] {
            return Err(anyhow!("Cannot calculate hash"));
        }

        debug!("Successfully downloaded {hash:?}");

        Ok(hash)
    }

    async fn insert_file_into_dir(
        &self,
        dir_stack: &mut DirStack<C>,
        parent: Option<DocId>,
        file_writer: c!(C::BlockstoreInterface::FileWriter),
        doc_id: &DocId,
        to_commit: &mut Vec<DocId>,
    ) -> Result<[u8; 32]> {
        let file_name = doc_id.file_name().unwrap_or_default();
        let temp_hash = file_writer.commit().await?;
        if let Some(parent) = parent {
            if let Some((dir, _)) = dir_stack.get_mut(&parent) {
                let entry = BorrowedEntry {
                    name: file_name.as_bytes(),
                    link: BorrowedLink::Content(&temp_hash),
                };
                dir.insert(entry, false).await?;

                if dir.ready_to_commit() {
                    to_commit.push(parent);
                }
            }
        }
        Ok(temp_hash)
    }
}
