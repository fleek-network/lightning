use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use affair::{Socket, Task};
use anyhow::{anyhow, Context, Result};
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::types::{
    Blake3Hash,
    FetcherRequest,
    FetcherResponse,
    ImmutablePointer,
    ServerRequest,
};
use lightning_interfaces::{
    BlockStoreInterface,
    BlockStoreServerInterface,
    BlockStoreServerSocket,
    ConfigConsumer,
    FetcherInterface,
    FetcherSocket,
    OriginProviderInterface,
    OriginProviderSocket,
    ResolverInterface,
    WithStartAndShutdown,
};
use tokio::sync::{mpsc, oneshot};
use tracing::error;

use crate::config::Config;
use crate::origin::{OriginFetcher, OriginRequest};

pub(crate) type Uri = Vec<u8>;

pub struct Fetcher<C: Collection> {
    inner: Arc<FetcherInner<C>>,
    is_running: Arc<AtomicBool>,
    socket: FetcherSocket,
    shutdown_tx: mpsc::Sender<()>,
}

impl<C: Collection> FetcherInterface<C> for Fetcher<C> {
    /// Initialize the fetcher.
    fn init(
        config: Self::Config,
        blockstore: C::BlockStoreInterface,
        blockstore_server: &C::BlockStoreServerInterface,
        resolver: C::ResolverInterface,
        origin: &C::OriginProviderInterface,
    ) -> anyhow::Result<Self> {
        let (socket, socket_rx) = Socket::raw_bounded(2048);
        let (shutdown_tx, shutdown_rx) = mpsc::channel(10);
        let inner = FetcherInner::<C> {
            config,
            socket_rx: Arc::new(Mutex::new(Some(socket_rx))),
            origin_socket: origin.get_socket(),
            blockstore,
            blockstore_server_socket: blockstore_server.get_socket(),
            resolver,
            shutdown_rx: Arc::new(Mutex::new(Some(shutdown_rx))),
        };

        Ok(Self {
            inner: Arc::new(inner),
            is_running: Arc::new(AtomicBool::new(false)),
            socket,
            shutdown_tx,
        })
    }

    fn get_socket(&self) -> FetcherSocket {
        self.socket.clone()
    }
}

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
            error!("Cannot start fetcher because it is already running");
        }
    }

    async fn shutdown(&self) {
        self.shutdown_tx
            .send(())
            .await
            .expect("Failed to send shutdown signal");
    }
}

struct FetcherInner<C: Collection> {
    config: Config,
    #[allow(clippy::type_complexity)]
    socket_rx: Arc<Mutex<Option<mpsc::Receiver<Task<FetcherRequest, FetcherResponse>>>>>,
    origin_socket: OriginProviderSocket,
    blockstore: C::BlockStoreInterface,
    blockstore_server_socket: BlockStoreServerSocket,
    resolver: C::ResolverInterface,
    shutdown_rx: Arc<Mutex<Option<mpsc::Receiver<()>>>>,
}

impl<C: Collection> FetcherInner<C> {
    async fn start(self: Arc<Self>) {
        let mut socket_rx = self.socket_rx.lock().unwrap().take().unwrap();

        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let (tx, rx) = mpsc::channel(128);
        let origin_fetcher = OriginFetcher::<C>::new(
            self.config.max_conc_origin_req,
            self.origin_socket.clone(),
            rx,
            self.resolver.clone(),
            shutdown_rx,
        );
        tokio::spawn(async move {
            origin_fetcher.start().await;
        });
        let mut shutdown_rx = self.shutdown_rx.lock().unwrap().take().unwrap();

        loop {
            tokio::select! {
                _ = shutdown_rx.recv() => {
                    shutdown_tx.send(()).expect("Failed to send shutdown signal to origin fetcher");
                    break;
                }
                task = socket_rx.recv() => {
                    let Some(task) = task else {
                        continue;
                    };
                    match task.request.clone() {
                        FetcherRequest::Put { pointer } => {
                            let tx = tx.clone();
                            let resolver = self.resolver.clone();
                            let blockstore = self.blockstore.clone();
                            let blockstore_server_socket = self.blockstore_server_socket.clone();
                            tokio::spawn(async move {
                                task.respond(FetcherResponse::Put(FetcherInner::<C>::put(
                                    pointer,
                                    tx,
                                    resolver,
                                    blockstore,
                                    blockstore_server_socket,
                                ).await));
                            });
                        }
                        FetcherRequest::Fetch { hash } => {
                            let tx = tx.clone();
                            let resolver = self.resolver.clone();
                            let blockstore = self.blockstore.clone();
                            let blockstore_server_socket = self.blockstore_server_socket.clone();
                            tokio::spawn(async move {
                                task.respond(FetcherResponse::Fetch(FetcherInner::<C>::fetch(
                                    hash,
                                    tx,
                                    resolver,
                                    blockstore,
                                    blockstore_server_socket,
                                ).await));
                            });
                        }
                    }
                }
            }
        }
        *self.socket_rx.lock().unwrap() = Some(socket_rx);
        *self.shutdown_rx.lock().unwrap() = Some(shutdown_rx);
    }

    /// Fetches the data from the corresponding origin, puts it in the blockstore, and stores the
    /// mapping using the resolver. If the mapping already exists, the data will not be fetched
    /// from origin again.
    async fn put(
        pointer: ImmutablePointer,
        tx: mpsc::Sender<OriginRequest>,
        resolver: C::ResolverInterface,
        blockstore: C::BlockStoreInterface,
        blockstore_server_socket: BlockStoreServerSocket,
    ) -> anyhow::Result<[u8; 32]> {
        if let Some(hash) = resolver.get_blake3_hash(pointer.clone()).await {
            // If we know about a mapping, forward the call to fetch, which will additionally pull
            // extra origins if the provider no longer has it.
            return FetcherInner::<C>::fetch(
                hash,
                tx,
                resolver,
                blockstore,
                blockstore_server_socket,
            )
            .await
            .map(|_| hash);
        }

        // Otherwise, use the origin as-is
        FetcherInner::<C>::fetch_origin(pointer, &tx).await
    }

    #[inline(always)]
    async fn fetch_origin(
        pointer: ImmutablePointer,
        tx: &mpsc::Sender<OriginRequest>,
    ) -> Result<[u8; 32]> {
        let (response_tx, response_rx) = oneshot::channel();
        tx.send(OriginRequest {
            pointer,
            response: response_tx,
        })
        .await
        .expect("Failed to send origin request");

        response_rx
            .await?
            .recv()
            .await?
            .context("failed to fetch from origin")
    }

    /// Fetches the data from the blockstore. If the data does not exist in the blockstore, it will
    /// be fetched from the origin.
    async fn fetch(
        hash: Blake3Hash,
        tx: mpsc::Sender<OriginRequest>,
        resolver: C::ResolverInterface,
        blockstore: C::BlockStoreInterface,
        blockstore_server_socket: BlockStoreServerSocket,
    ) -> Result<()> {
        if blockstore.get_tree(&hash).await.is_some() {
            return Ok(());
        } else if let Some(pointers) = resolver.get_origins(hash) {
            for res_pointer in pointers {
                if res_pointer.hash == hash && blockstore.get_tree(&hash).await.is_some() {
                    return Ok(());
                }
                // Try to get the content from the peer that advertized the record.
                let mut res = blockstore_server_socket
                    .run(ServerRequest {
                        hash,
                        peer: res_pointer.originator,
                    })
                    .await
                    .expect("Failed to send request to blockstore server");
                if let Ok(()) = res
                    .recv()
                    .await
                    .expect("Failed to receive response from blockstore server")
                {
                    return Ok(());
                }

                if FetcherInner::<C>::fetch_origin(res_pointer.pointer, &tx)
                    .await
                    .is_ok()
                {
                    return Ok(());
                }
            }
        }
        Err(anyhow!("Failed to resolve hash"))
    }
}

impl<C: Collection> ConfigConsumer for Fetcher<C> {
    const KEY: &'static str = "fetcher";

    type Config = Config;
}
