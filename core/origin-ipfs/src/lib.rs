use std::marker::PhantomData;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use affair::{Socket, Task};
use anyhow::{anyhow, Context};
use async_trait::async_trait;
use cid::multihash::{Code, MultihashDigest};
use cid::Cid;
use hyper::client::{self, HttpConnector};
use hyper::{Body, Client, Request, Uri};
use hyper_rustls::{ConfigBuilderExt, HttpsConnector};
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::types::CompressionAlgorithm;
use lightning_interfaces::{
    Blake3Hash,
    BlockStoreInterface,
    ConfigConsumer,
    IncrementalPutInterface,
    OriginProviderInterface,
    OriginProviderSocket,
    WithStartAndShutdown,
};
use log::{error, info};
use tokio::io::AsyncReadExt;
use tokio::sync::{mpsc, Notify};
use tokio::time::timeout;
mod config;
use config::{Config, Gateway};
mod ipfs_stream;
pub use ipfs_stream::IPFSStream;
use tokio_util::io::StreamReader;
#[cfg(test)]
mod tests;

const GATEWAY_TIMEOUT: Duration = Duration::from_millis(500);

#[allow(clippy::type_complexity)]
pub struct IPFSOrigin<C: Collection> {
    inner: Arc<IPFSOriginInner<C>>,
    socket: OriginProviderSocket,
    rx: Arc<Mutex<Option<mpsc::Receiver<Task<Vec<u8>, anyhow::Result<Blake3Hash>>>>>>,
    is_running: Arc<Mutex<bool>>,
    shutdown_notify: Arc<Notify>,
    collection: PhantomData<C>,
}

#[async_trait]
impl<C: Collection> OriginProviderInterface<C> for IPFSOrigin<C> {
    type Stream = IPFSStream;
    fn init(config: Config, blockstore: C::BlockStoreInterface) -> anyhow::Result<Self> {
        let (socket, rx) = Socket::raw_bounded(2048);
        let inner = IPFSOriginInner {
            gateways: config.gateways,
            blockstore,
        };

        Ok(IPFSOrigin {
            inner: Arc::new(inner),
            socket,
            rx: Arc::new(Mutex::new(Some(rx))),
            is_running: Arc::new(Mutex::new(false)),
            shutdown_notify: Arc::new(Notify::new()),
            collection: PhantomData,
        })
    }

    fn get_socket(&self) -> OriginProviderSocket {
        self.socket.clone()
    }
}

#[async_trait]
impl<C: Collection> WithStartAndShutdown for IPFSOrigin<C> {
    fn is_running(&self) -> bool {
        *self.is_running.lock().unwrap()
    }

    async fn start(&self) {
        if !*self.is_running.lock().unwrap() {
            let inner = self.inner.clone();
            let rx = self.rx.lock().unwrap().take().unwrap();
            let shutdown_notify = self.shutdown_notify.clone();
            tokio::spawn(async move { inner.handle(rx, shutdown_notify).await });
            *self.is_running.lock().unwrap() = true;
        }
    }

    async fn shutdown(&self) {
        self.shutdown_notify.notify_one();
        *self.is_running.lock().unwrap() = false;
    }
}

struct IPFSOriginInner<C: Collection> {
    gateways: Vec<Gateway>,
    blockstore: C::BlockStoreInterface,
}

impl<C: Collection> IPFSOriginInner<C> {
    async fn handle(
        self: Arc<Self>,
        mut rx: mpsc::Receiver<Task<Vec<u8>, anyhow::Result<Blake3Hash>>>,
        shutdown_notify: Arc<Notify>,
    ) {
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
        let client: client::Client<_, hyper::Body> = client::Client::builder().build(https);

        'outer: loop {
            tokio::select! {
                task = rx.recv() => {
                    let Some(task) = task else {
                        error!("Failed to receive task");
                        continue;
                    };
                    match self.fetch(&client, &task.request).await {
                        Ok(stream) => {
                            let mut blockstore_putter = self.blockstore.put(None);
                            let mut read = StreamReader::new(stream);
                            let mut buf = [0; 1024];
                            let mut bytes: Vec<u8> = Vec::new();
                            let comp = CompressionAlgorithm::Uncompressed; // clippy
                            loop {
                                let len = read.read(&mut buf).await.unwrap();
                                if let Err(e) = blockstore_putter.write(&buf[..len], comp){
                                    task.respond(Err(anyhow!("Failed to write to the blockstore: {e}")));
                                    continue 'outer;
                                }
                                bytes.extend(&buf[..len]);
                                if len == 0 {
                                    break;
                                }
                            }
                            let Ok(requested_cid) = Cid::try_from(task.request.clone()) else {
                                error!("Failed to parse uri into cid");
                                continue;
                            };
                            let valid = match Code::try_from(requested_cid.hash().code()) {
                                Ok(hasher) => &hasher.digest(&bytes) == requested_cid.hash(),
                                _ => false,
                            };
                            if valid {
                                let hash = blockstore_putter.finalize().await.map_err(|e| anyhow!("failed to finalize in blockstore: {e}"));
                                task.respond(hash);
                            } else {
                                task.respond(Err(anyhow!("Data verification failed")));
                            }
                        },
                        Err(e) => task.respond(Err(e)),
                    }
                }
                _ = shutdown_notify.notified() => break,
            }
        }
    }

    async fn fetch(
        &self,
        client: &Client<HttpsConnector<HttpConnector>, hyper::Body>,
        uri: &[u8],
    ) -> anyhow::Result<IPFSStream> {
        let requested_cid = Cid::try_from(uri).with_context(|| "Failed to parse uri into cid")?;

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

            match timeout(GATEWAY_TIMEOUT, client.request(req)).await {
                Ok(Ok(res)) => {
                    let body = res.into_body();
                    return Ok(IPFSStream::new(requested_cid, body));
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

impl<C: Collection> ConfigConsumer for IPFSOrigin<C> {
    const KEY: &'static str = "origin-ipfs";

    type Config = Config;
}
