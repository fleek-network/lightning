use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use affair::{Socket, Task};
use anyhow::{anyhow, Context};
use async_trait::async_trait;
use cid::Cid;
use hyper::{
    client::{self, HttpConnector},
    Body, Client, Request, Uri,
};
use hyper_rustls::{ConfigBuilderExt, HttpsConnector};
use lightning_interfaces::{
    ConfigConsumer, OriginProviderInterface, OriginProviderSocket, WithStartAndShutdown,
};
use tokio::{
    sync::{mpsc, Notify},
    time::timeout,
};

mod config;
use config::Config;
mod ipfs_stream;
use ipfs_stream::IPFSStream;
#[cfg(test)]
mod tests;

const GATEWAY_TIMEOUT: Duration = Duration::from_millis(500);

#[allow(clippy::type_complexity)]
pub struct IPFSOrigin {
    inner: Arc<IPFSOriginInner>,
    socket: Socket<Vec<u8>, anyhow::Result<IPFSStream>>,
    rx: Arc<Mutex<Option<mpsc::Receiver<Task<Vec<u8>, anyhow::Result<IPFSStream>>>>>>,
    is_running: Arc<Mutex<bool>>,
    shutdown_notify: Arc<Notify>,
}

#[async_trait]
impl OriginProviderInterface<IPFSStream> for IPFSOrigin {
    async fn init(config: Config) -> anyhow::Result<Self> {
        let (socket, rx) = Socket::raw_bounded(2048);
        let inner = IPFSOriginInner {
            gateways: config.gateways,
        };

        Ok(IPFSOrigin {
            inner: Arc::new(inner),
            socket,
            rx: Arc::new(Mutex::new(Some(rx))),
            is_running: Arc::new(Mutex::new(false)),
            shutdown_notify: Arc::new(Notify::new()),
        })
    }

    fn get_socket(&self) -> OriginProviderSocket<IPFSStream> {
        self.socket.clone()
    }
}

#[async_trait]
impl WithStartAndShutdown for IPFSOrigin {
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

struct IPFSOriginInner {
    gateways: Vec<String>,
}

impl IPFSOriginInner {
    async fn handle(
        self: Arc<Self>,
        mut rx: mpsc::Receiver<Task<Vec<u8>, anyhow::Result<IPFSStream>>>,
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

        loop {
            tokio::select! {
                task = rx.recv() => {
                    let task = task.expect("Failed to receive fetch request.");
                    let stream = self.fetch(&client, &task.request).await;
                    task.respond(stream);
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
                .scheme("https")
                .authority(gateway.as_str())
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
                Ok(Err(err)) => return Err(anyhow!("Failed to fetch from IPFS gateway: {err:?}")),
                Err(_) => {
                    // timeout, onto the next gateway
                    continue;
                },
            }
        }
        Err(anyhow::anyhow!("No response from gateways."))
    }
}

impl ConfigConsumer for IPFSOrigin {
    const KEY: &'static str = "origin-ipfs";

    type Config = Config;
}
