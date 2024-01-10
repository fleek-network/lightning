use std::collections::HashMap;
use std::sync::Arc;

use affair::Task;
use lightning_interfaces::infu_collection::{c, Collection};
use lightning_interfaces::types::Blake3Hash;
use lightning_origin_http::HttpOriginFetcher;
use tokio::sync::mpsc::Receiver;
use tokio::sync::Notify;
use tokio::task::JoinHandle;

pub const HTTP_ORIGIN: &str = "http";

pub struct Muxer<C: Collection> {
    origins: HashMap<&'static str, Origin<C>>,
    task_rx: Receiver<Task<Vec<u8>, anyhow::Result<Blake3Hash>>>,
}

impl<C: Collection> Muxer<C> {
    pub fn new(task_rx: Receiver<Task<Vec<u8>, anyhow::Result<Blake3Hash>>>) -> Self {
        Self {
            origins: HashMap::new(),
            task_rx,
        }
    }

    pub fn http_origin(&mut self, origin: HttpOriginFetcher<C>) {
        self.origins.insert(HTTP_ORIGIN, Origin::Http(origin));
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

    async fn start(&mut self, shutdown: Arc<Notify>) {
        loop {
            tokio::select! {
                biased;
                _ = shutdown.notified() => {
                    break;
                }
                task = self.task_rx.recv() => {
                    todo!("handle task")
                }
            }
        }
    }
}

// Todo: try using enum_dispatch.
pub enum Origin<C: Collection> {
    Http(HttpOriginFetcher<C>),
    Ipfs,
}
