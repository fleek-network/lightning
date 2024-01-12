use std::sync::Arc;

use affair::Task;
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::types::{Blake3Hash, ImmutablePointer, OriginProvider};
use lightning_origin_http::HttpOriginFetcher;
use lightning_origin_ipfs::IPFSOrigin;
use tokio::sync::mpsc::Receiver;
use tokio::sync::Notify;
use tokio::task::JoinHandle;

use crate::Config;

pub struct Demuxer<C: Collection> {
    http: HttpOriginFetcher<C>,
    ipfs: IPFSOrigin<C>,
    task_rx: Receiver<Task<ImmutablePointer, anyhow::Result<Blake3Hash>>>,
}

impl<C: Collection> Demuxer<C> {
    pub fn new(
        config: Config,
        blockstore: C::BlockStoreInterface,
        task_rx: Receiver<Task<ImmutablePointer, anyhow::Result<Blake3Hash>>>,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            http: HttpOriginFetcher::<C>::new(config.http, blockstore.clone())?,
            ipfs: IPFSOrigin::<C>::new(config.ipfs, blockstore)?,
            task_rx,
        })
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
        match &task.request.origin {
            OriginProvider::HTTP => {
                let fetcher = self.http.clone();
                tokio::spawn(async move {
                    let hash = fetcher.fetch(&task.request.uri).await;
                    task.respond(hash);
                });
            },
            OriginProvider::IPFS => {
                let fetcher = self.ipfs.clone();
                tokio::spawn(async move {
                    let hash = fetcher.fetch(&task.request.uri).await;
                    task.respond(hash);
                });
            },
            _ => {
                task.respond(Err(anyhow::anyhow!("unknown origin type")));
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
