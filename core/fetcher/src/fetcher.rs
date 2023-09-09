use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use affair::{Socket, Task};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::types::{
    Blake3Hash,
    FetcherRequest,
    FetcherResponse,
    ImmutablePointer,
    OriginProvider,
};
use lightning_interfaces::{
    BlockStoreInterface,
    ConfigConsumer,
    FetcherInterface,
    FetcherSocket,
    OriginProviderInterface,
    OriginProviderSocket,
    ResolverInterface,
    WithStartAndShutdown,
};
use log::error;
use tokio::sync::{mpsc, Notify};

use crate::config::Config;

#[derive(Clone)]
pub struct Fetcher<C: Collection> {
    inner: Arc<FetcherInner<C>>,
    is_running: Arc<AtomicBool>,
    socket: FetcherSocket,
    shutdown_notify: Arc<Notify>,
}

#[async_trait]
impl<C: Collection> FetcherInterface<C> for Fetcher<C> {
    /// Initialize the fetcher.
    fn init(
        _config: Self::Config,
        blockstore: C::BlockStoreInterface,
        resolver: C::ResolverInterface,
        origin: &C::OriginProviderInterface,
    ) -> anyhow::Result<Self> {
        let (socket, socket_rx) = Socket::raw_bounded(2048);
        let shutdown_notify = Arc::new(Notify::new());
        let inner = FetcherInner::<C> {
            socket_rx: Arc::new(Mutex::new(Some(socket_rx))),
            origin_socket: origin.get_socket(),
            blockstore,
            resolver,
            shutdown_notify: shutdown_notify.clone(),
        };

        Ok(Self {
            inner: Arc::new(inner),
            is_running: Arc::new(AtomicBool::new(false)),
            socket,
            shutdown_notify,
        })
    }

    fn get_socket(&self) -> FetcherSocket {
        self.socket.clone()
    }
}

#[async_trait]
impl<C: Collection> WithStartAndShutdown for Fetcher<C> {
    fn is_running(&self) -> bool {
        self.is_running.load(Ordering::Relaxed)
    }

    async fn start(&self) {
        if !self.is_running() {
            let inner = self.inner.clone();
            let is_running = self.is_running.clone();
            tokio::spawn(async move {
                inner.start().await;
                is_running.store(false, Ordering::Relaxed);
            });
            self.is_running.store(true, Ordering::Relaxed);
        } else {
            error!("Cannot start reputation aggregator because it is already running");
        }
    }

    async fn shutdown(&self) {
        self.shutdown_notify.notify_one();
    }
}

struct FetcherInner<C: Collection> {
    #[allow(clippy::type_complexity)]
    socket_rx: Arc<Mutex<Option<mpsc::Receiver<Task<FetcherRequest, FetcherResponse>>>>>,
    origin_socket: OriginProviderSocket,
    blockstore: C::BlockStoreInterface,
    resolver: C::ResolverInterface,
    shutdown_notify: Arc<Notify>,
}

impl<C: Collection> FetcherInner<C> {
    async fn start(&self) {
        let mut socket_rx = self.socket_rx.lock().unwrap().take().unwrap();
        let shutdown_notify = self.shutdown_notify.clone();
        loop {
            tokio::select! {
                _ = shutdown_notify.notified() => {
                    break;
                }
                task = socket_rx.recv() => {
                    let task = task.expect("Failed to receive UpdateMethod.");
                    match task.request.clone() {
                        FetcherRequest::Put { pointer } => {
                            task.respond(FetcherResponse::Put(self.put(pointer).await));
                        }
                        FetcherRequest::Fetch { hash } => {
                            task.respond(FetcherResponse::Fetch(self.fetch(hash).await));
                        }
                    }

                }
            }
        }
        *self.socket_rx.lock().unwrap() = Some(socket_rx);
    }

    /// Fetches the data from the corresponding origin, puts it in the blockstore, and stores the
    /// mapping using the resolver. If the mapping already exists, the data will not be fetched
    /// from origin again.
    async fn put(&self, pointer: ImmutablePointer) -> Result<Blake3Hash> {
        match pointer.origin {
            OriginProvider::IPFS => {
                if let Some(resolved_pointer) = self.resolver.get_blake3_hash(pointer.clone()).await
                {
                    Ok(resolved_pointer.hash)
                } else {
                    let Ok(Ok(hash)) = self.origin_socket.run(pointer.uri.clone()).await else {
                        return Err(anyhow!("Failed to get response"));
                    };
                    // TODO(matthias): use different hashing algos
                    self.resolver.publish(hash, &[pointer]).await;
                    Ok(hash)
                }
            },
            _ => unreachable!(),
        }
    }

    /// Fetches the data from the blockstore. If the data does not exist in the blockstore, it will
    /// be fetched from the origin.
    async fn fetch(&self, hash: Blake3Hash) -> Result<()> {
        if self.blockstore.get_tree(&hash).await.is_some() {
            return Ok(());
        }
        if let Some(pointers) = self.resolver.get_origins(hash) {
            for resolved_pointer in pointers {
                if let Ok(res_hash) = self.put(resolved_pointer.pointer).await {
                    if res_hash == hash && self.blockstore.get_tree(&hash).await.is_some() {
                        return Ok(());
                    }
                }
            }
        }

        Err(anyhow!("Failed to fetch data"))
    }
}

impl<C: Collection> ConfigConsumer for Fetcher<C> {
    const KEY: &'static str = "fetcher";

    type Config = Config;
}
