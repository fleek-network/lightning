use std::marker::PhantomData;

use affair::{AsyncWorkerUnordered, Executor, TokioSpawn};
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
    fdi,
    BlockstoreInterface,
    BlockstoreServerInterface,
    BlockstoreServerSocket,
    ConfigConsumer,
    ConfigProviderInterface,
    FetcherInterface,
    FetcherSocket,
    OriginProviderInterface,
    ResolverInterface,
};
use lightning_metrics::increment_counter;
use tokio::sync::{mpsc, oneshot};

use crate::config::Config;
use crate::origin::{OriginFetcher, OriginRequest};

pub(crate) type Uri = Vec<u8>;

pub struct Fetcher<C: Collection> {
    socket: FetcherSocket,
    _collection: PhantomData<C>,
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

        tokio::spawn(async move {
            waiter.run_until_shutdown(origin_fetcher.start()).await;
        });

        let worker = FetcherWorker::<C> {
            origin_tx,
            blockstore,
            blockstore_server_socket: blockstore_server.get_socket(),
            resolver,
        };

        let socket = TokioSpawn::spawn_async_unordered(worker);

        Ok(Self {
            socket,
            _collection: PhantomData,
        })
    }
}

impl<C: Collection> FetcherInterface<C> for Fetcher<C> {
    fn get_socket(&self) -> FetcherSocket {
        self.socket.clone()
    }
}

struct FetcherWorker<C: Collection> {
    origin_tx: mpsc::Sender<OriginRequest>,
    blockstore: C::BlockstoreInterface,
    blockstore_server_socket: BlockstoreServerSocket,
    resolver: C::ResolverInterface,
}

impl<C: Collection> FetcherWorker<C> {
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
        fdi::DependencyGraph::new().with(Self::new)
    }
}

impl<C: Collection> AsyncWorkerUnordered for FetcherWorker<C> {
    type Request = FetcherRequest;
    type Response = FetcherResponse;

    async fn handle(&self, req: Self::Request) -> Self::Response {
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
                FetcherResponse::Put(res)
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
                FetcherResponse::Fetch(res)
            },
        }
    }
}
