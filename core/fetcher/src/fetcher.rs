use affair::{Socket, Task};
use anyhow::{anyhow, Context, Result};
use lightning_interfaces::prelude::*;
use lightning_interfaces::types::{
    Blake3Hash,
    FetcherRequest,
    FetcherResponse,
    ImmutablePointer,
    ServerRequest,
};
use lightning_interfaces::{BlockstoreServerSocket, FetcherSocket};
use lightning_metrics::increment_counter;
use tokio::sync::{mpsc, oneshot};

use crate::config::Config;
use crate::origin::{OriginFetcher, OriginRequest};

pub(crate) type Uri = Vec<u8>;

pub struct Fetcher<C: Collection> {
    socket: FetcherSocket,
    inner: Option<FetcherInner<C>>,
}

impl<C: Collection> Fetcher<C> {
    /// Initialize the fetcher.
    pub fn new(
        config: &C::ConfigProviderInterface,
        blockstore_server: &C::BlockstoreServerInterface,
        origin: &C::OriginProviderInterface,
        fdi::Cloned(waiter): fdi::Cloned<lightning_interfaces::ShutdownWaiter>,
        fdi::Cloned(blockstore): fdi::Cloned<C::BlockstoreInterface>,
        fdi::Cloned(resolver): fdi::Cloned<C::ResolverInterface>,
    ) -> anyhow::Result<Self> {
        let config = config.get::<Self>();

        let (origin_tx, rx) = mpsc::channel(128);
        let origin_fetcher = OriginFetcher::<C>::new(
            config.max_conc_origin_req,
            origin.get_socket(),
            rx,
            resolver.clone(),
        );

        let inner_waiter = waiter.clone();
        // TODO(matthias): make this a crucial task?
        spawn!(
            async move {
                waiter.run_until_shutdown(origin_fetcher.start()).await;
            },
            "FETCHER: shutdown waiter"
        );

        let (socket, socket_rx) = Socket::raw_bounded(128);

        let inner = FetcherInner::<C>::new(
            socket_rx,
            origin_tx,
            blockstore,
            blockstore_server.get_socket(),
            resolver,
            inner_waiter,
        );

        Ok(Self {
            inner: Some(inner),
            socket,
        })
    }

    pub async fn start(mut this: fdi::RefMut<Self>) {
        this.inner
            .take()
            .expect("Fetcher already started")
            .run()
            .await;
    }
}

impl<C: Collection> FetcherInterface<C> for Fetcher<C> {
    fn get_socket(&self) -> FetcherSocket {
        self.socket.clone()
    }
}

struct FetcherInner<C: Collection> {
    socket_rx: mpsc::Receiver<Task<FetcherRequest, FetcherResponse>>,
    origin_tx: mpsc::Sender<OriginRequest>,
    shutdown_waiter: ShutdownWaiter,
    blockstore: C::BlockstoreInterface,
    blockstore_server_socket: BlockstoreServerSocket,
    resolver: C::ResolverInterface,
}

impl<C: Collection> FetcherInner<C> {
    fn new(
        socket_rx: mpsc::Receiver<Task<FetcherRequest, FetcherResponse>>,
        origin_tx: mpsc::Sender<OriginRequest>,
        blockstore: C::BlockstoreInterface,
        blockstore_server_socket: BlockstoreServerSocket,
        resolver: C::ResolverInterface,
        shutdown_waiter: ShutdownWaiter,
    ) -> Self {
        Self {
            socket_rx,
            origin_tx,
            blockstore,
            blockstore_server_socket,
            resolver,
            shutdown_waiter,
        }
    }

    async fn run(mut self) {
        loop {
            tokio::select! {
                biased;
                _ = self.shutdown_waiter.wait_for_shutdown() => {
                    return;
                }
                task = self.socket_rx.recv() => {
                    let Some(task) = task else {
                        break;
                    };
                    let req = task.request.clone();
                    match req {
                        FetcherRequest::Put { pointer } => {
                            let res = self.put(pointer).await;
                            if res.is_err() {
                                increment_counter!(
                                    "fetcher_put_request_failed",
                                    Some("Counter for failed put requests for origin content")
                                );
                            } else {
                                increment_counter!(
                                    "fetcher_put_request_succeed",
                                    Some("Counter for successful put requests for origin content")
                                );
                            }
                            task.respond(FetcherResponse::Put(res));
                        },
                        FetcherRequest::Fetch { hash } => {
                            let res = self.fetch(hash).await;
                            if res.is_err() {
                                increment_counter!(
                                    "fetcher_fetch_request_failed",
                                    Some("Counter for failed fetch requests for native content")
                                );
                            } else {
                                increment_counter!(
                                    "fetcher_fetch_request_succeed",
                                    Some("Counter for successful fetch requests for native content")
                                );
                            }
                            task.respond(FetcherResponse::Fetch(res));
                        },
                    }
                }
            }
        }
    }

    /// Fetches the data from the corresponding origin, puts it in the blockstore,
    /// and stores the mapping using the resolver. If pulling a origin fails, it will not ,
    /// the data will not be fetched from origin again.
    #[inline(always)]
    async fn put(&self, pointer: ImmutablePointer) -> anyhow::Result<[u8; 32]> {
        if let Some(hash) = self.resolver.get_blake3_hash(pointer.clone()).await {
            // If we know about a mapping, forward the call to fetch which
            // will attempt to pull from multiple sources.
            return self.fetch(hash).await.map(|_| hash);
        }

        // Otherwise, try to fetch directly from the origin
        self.fetch_origin(pointer).await
    }

    #[inline(always)]
    async fn fetch_origin(&self, pointer: ImmutablePointer) -> Result<[u8; 32]> {
        let (response_tx, response_rx) = oneshot::channel();
        self.origin_tx
            .send(OriginRequest {
                pointer,
                response: response_tx,
            })
            .await
            .expect("Failed to send origin request");

        let mut res = response_rx.await?;
        let res = res.recv().await?.context("failed to fetch from origin");

        if res.is_ok() {
            increment_counter!(
                "fetcher_from_origin",
                Some("Counter for content that was fetched from an origin")
            );
        } else {
            increment_counter!(
                "fetcher_from_origin_failed",
                Some("Counter for a failed attempts to fetch from an origin")
            );
        }

        res
    }

    /// Attempt to fetch the blake3 content. First, we check the blockstore,
    /// then iterate through the provider records, requesting from the provider,
    /// then falling back to the record's immutable pointer.
    #[inline(always)]
    async fn fetch(&self, hash: Blake3Hash) -> Result<()> {
        if self.blockstore.get_tree(&hash).await.is_some() {
            increment_counter!(
                "fetcher_from_cache",
                Some("Counter for content that was already cached locally")
            );
            return Ok(());
        } else if let Some(pointers) = self.resolver.get_origins(hash) {
            for res_pointer in pointers {
                debug_assert_eq!(res_pointer.hash, hash);
                if self.blockstore.get_tree(&hash).await.is_some() {
                    // in case we have the file
                    increment_counter!(
                        "fetcher_from_cache",
                        Some("Counter for content that was already cached locally")
                    );
                    return Ok(());
                }

                // Try to get the content from the peer that advertised the record.
                // TODO: Maybe use indexer for getting peers instead
                let mut res = self
                    .blockstore_server_socket
                    .run(ServerRequest {
                        hash,
                        peer: res_pointer.originator,
                    })
                    .await
                    .expect("Failed to send request to blockstore server");
                let res = res
                    .recv()
                    .await
                    .expect("Failed to receive response from blockstore server");

                if res.is_ok() {
                    increment_counter!(
                        "fetcher_from_peer",
                        Some("Counter for content that was fetched from a peer")
                    );
                    return Ok(());
                } else {
                    increment_counter!(
                        "fetcher_from_peer_failed",
                        Some("Counter for failed attempts to fetch from a peer")
                    );
                }

                // If not, attempt to pull from the origin. This strikes a balance between trying
                // to fetch from a bunch of peers vs going to the origin right away.
                if self.fetch_origin(res_pointer.pointer).await.is_ok() {
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

impl<C: Collection> fdi::BuildGraph for Fetcher<C> {
    fn build_graph() -> fdi::DependencyGraph {
        fdi::DependencyGraph::new().with(
            Self::new.with_event_handler("start", Self::start.wrap_with_spawn_named("FETCHER")),
        )
    }
}
