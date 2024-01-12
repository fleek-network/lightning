use std::collections::HashMap;
use std::sync::Arc;

use affair::Task;
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::types::{Blake3Hash, ImmutablePointer, OriginProvider};
use lightning_interfaces::OriginFetcherInterface;
use lightning_origin_http::HttpOriginFetcher;
use lightning_origin_ipfs::IPFSOrigin;
use tokio::sync::mpsc::Receiver;
use tokio::sync::Notify;
use tokio::task::JoinHandle;

pub struct Muxer<C: Collection> {
    origins: HashMap<OriginProvider, Origin<C>>,
    task_rx: Receiver<Task<ImmutablePointer, anyhow::Result<Blake3Hash>>>,
}

impl<C: Collection> Muxer<C> {
    pub fn new(task_rx: Receiver<Task<ImmutablePointer, anyhow::Result<Blake3Hash>>>) -> Self {
        Self {
            origins: HashMap::new(),
            task_rx,
        }
    }

    pub fn http_origin(&mut self, origin: HttpOriginFetcher<C>) {
        self.origins
            .insert(OriginProvider::HTTP, Origin::Http(origin));
    }

    pub fn ipfs_origin(&mut self, origin: IPFSOrigin<C>) {
        self.origins
            .insert(OriginProvider::IPFS, Origin::Ipfs(origin));
    }

    pub fn spawn(mut self) -> (JoinHandle<Self>, Arc<Notify>) {
        let shutdown = Arc::new(Notify::new());
        let shutdown_clone = shutdown.clone();
        let handle = tokio::spawn(async move {
            self.start(shutdown_clone).await;
            self
        });
        (handle, shutdown)
    }

    fn handle(&mut self, task: Task<ImmutablePointer, anyhow::Result<Blake3Hash>>) {
        match self.origins.get(&task.request.origin) {
            None => {
                task.respond(Err(anyhow::anyhow!("unknown origin type")));
            },
            Some(Origin::Http(origin)) => {
                let fetcher = origin.clone();
                tokio::spawn(async move {
                    let hash = fetcher.fetch(&task.request.uri).await;
                    task.respond(hash);
                });
            },
            Some(Origin::Ipfs(origin)) => {
                let fetcher = origin.clone();
                tokio::spawn(async move {
                    let hash = fetcher.fetch(&task.request.uri).await;
                    task.respond(hash);
                });
            },
        }
    }

    async fn start(&mut self, shutdown: Arc<Notify>) {
        loop {
            tokio::select! {
                biased;
                _ = shutdown.notified() => {
                    break;
                }
                task = self.task_rx.recv() => {
                    let Some(task) = task else {
                        break;
                    };
                    self.handle(task);
                }
            }
        }
    }
}

enum Origin<C: Collection> {
    Http(HttpOriginFetcher<C>),
    Ipfs(IPFSOrigin<C>),
}
