use std::marker::PhantomData;

use affair::AsyncWorkerUnordered;
use anyhow::{anyhow, Context, Result};
use lightning_interfaces::prelude::*;
use lightning_interfaces::types::{
    Blake3Hash,
    FetcherRequest,
    FetcherResponse,
    ImmutablePointer,
    ServerRequest,
};
use lightning_interfaces::{spawn_worker, BlockstoreServerSocket, FetcherSocket};
use lightning_metrics::increment_counter;
use tokio::sync::{mpsc, oneshot};
use tracing::info;
use types::{NodeIndex, PeerRequestError};

use crate::config::Config;
use crate::origin::{OriginError, OriginFetcher, OriginRequest};

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
        app: &C::ApplicationInterface,
        fdi::Cloned(blockstore): fdi::Cloned<C::BlockstoreInterface>,
        fdi::Cloned(resolver): fdi::Cloned<C::ResolverInterface>,
        fdi::Cloned(shutdown): fdi::Cloned<ShutdownWaiter>,
    ) -> anyhow::Result<Self> {
        let config = config.get::<Self>();

        let (origin_tx, rx) = mpsc::channel(128);
        let origin_fetcher = OriginFetcher::<C>::new(
            config.max_conc_origin_req,
            origin.get_socket(),
            rx,
            resolver.clone(),
        );

        let waiter = shutdown.clone();
        let app_query = app.sync_query();

        spawn!(
            async move {
                waiter
                    .run_until_shutdown(origin_fetcher.start(app_query))
                    .await;
            },
            "FETCHER: origin fetcher"
        );

        let worker = FetcherWorker::<C> {
            origin_tx,
            blockstore,
            blockstore_server_socket: blockstore_server.get_socket(),
            resolver,
            query_runner: app.sync_query(),
        };

        let socket = spawn_worker!(worker, "FETCHER", shutdown, crucial);

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
    query_runner: c!(C::ApplicationInterface::SyncExecutor),
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
        self.fetch_from_origin(pointer).await
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
        }
        let mut origin_pointers = self
            .resolver
            .get_origins(hash)
            .unwrap_or_default()
            .into_iter();
        let mut peers = self
            .query_runner
            .get_uri_providers(&hash)
            .unwrap_or_default()
            .into_iter();
        // TODO(matthias): more optimizations here are possible.
        // For example, we can send concurrent requests to multiple peers and or multiple origins.
        // Also, the list of peers would ideally be sorted by the latency to the local node.
        loop {
            let peer = peers.next();
            let pointer = origin_pointers.next();
            if peer.is_none() && pointer.is_none() {
                break;
            }
            if let Some(peer) = peer {
                // Try to get the content from the peer that advertised the record.
                if self.fetch_from_peer(peer, hash).await.is_ok() {
                    return Ok(());
                }
            }
            if let Some(pointer) = pointer {
                debug_assert_eq!(pointer.hash, hash);
                if self.blockstore.get_tree(&hash).await.is_some() {
                    // in case we have the file
                    increment_counter!(
                        "fetcher_from_cache",
                        Some("Counter for content that was already cached locally")
                    );
                    return Ok(());
                }
                // If not, attempt to pull from the origin. This strikes a balance between trying
                // to fetch from a bunch of peers vs going to the origin right away.
                if self.fetch_from_origin(pointer.pointer).await.is_ok() {
                    return Ok(());
                }
            }
        }
        Err(anyhow!("Failed to resolve hash"))
    }

    #[inline(always)]
    async fn fetch_from_origin(&self, pointer: ImmutablePointer) -> Result<[u8; 32]> {
        let (response_tx, response_rx) = oneshot::channel();

        #[inline(always)]
        fn emit_failed_metric() {
            increment_counter!(
                "fetcher_from_origin_failed",
                Some("Counter for a failed attempts to fetch from an origin")
            );
        }

        #[inline(always)]
        async fn recv(
            rx: oneshot::Receiver<tokio::sync::broadcast::Receiver<Result<[u8; 32], OriginError>>>,
        ) -> Result<[u8; 32]> {
            rx.await?.recv().await?.map_err(|e| e.into())
        }

        let res = self
            .origin_tx
            .send(OriginRequest {
                pointer,
                response: response_tx,
            })
            .await;

        match res {
            Ok(_) => match recv(response_rx).await {
                Ok(res) => {
                    increment_counter!(
                        "fetcher_from_origin",
                        Some("Counter for content that was fetched from an origin")
                    );
                    Ok(res)
                },
                Err(err) => {
                    info!("Failed to receive response from origin. Error: {:?}", err);
                    emit_failed_metric();
                    Err(err).context("Failed to fetch from origin")
                },
            },
            Err(err) => {
                info!("Failed to send origin request. Error: {:?}", err);
                emit_failed_metric();
                Err(err).context("Failed to send origin request")
            },
        }
    }

    #[inline(always)]
    async fn fetch_from_peer(&self, peer: NodeIndex, hash: Blake3Hash) -> Result<()> {
        #[inline(always)]
        fn emit_failed_metric() {
            increment_counter!(
                "fetcher_from_peer_failed",
                Some("Counter for failed attempts to fetch from a peer")
            );
        }

        #[inline(always)]
        async fn recv(
            mut res: tokio::sync::broadcast::Receiver<Result<(), PeerRequestError>>,
        ) -> Result<()> {
            res.recv().await?.map_err(|e| e.into())
        }

        let res = self
            .blockstore_server_socket
            .run(ServerRequest { hash, peer })
            .await;
        match res {
            Ok(res) => match recv(res).await {
                Ok(_) => {
                    increment_counter!(
                        "fetcher_from_peer",
                        Some("Counter for content that was fetched from a peer")
                    );
                    Ok(())
                },
                Err(err) => {
                    info!(
                        "Failed to receive response from blockstore server {:?}. Error: {:?}",
                        peer, err
                    );
                    emit_failed_metric();
                    Err(err).context("Failed to receive response from blockstore server")
                },
            },
            Err(err) => {
                info!(
                    "Failed to send request to blockstore server {:?}. Error: {:?}",
                    peer, err
                );
                emit_failed_metric();
                Err(anyhow!("Failed to send request to blockstore server"))
            },
        }
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
