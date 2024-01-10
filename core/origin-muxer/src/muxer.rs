use std::collections::HashMap;
use std::sync::Arc;

use affair::Task;
use lightning_interfaces::infu_collection::{c, Collection};
use lightning_interfaces::types::Blake3Hash;
use lightning_origin_http::HttpOriginFetcher;
use tokio::sync::mpsc::Receiver;
use tokio::sync::Notify;
use tokio::task::JoinHandle;

pub struct Muxer<C: Collection> {
    http_origin: Arc<HttpOriginFetcher<C>>,
    task_rx: Receiver<Task<Vec<u8>, anyhow::Result<Blake3Hash>>>,
}

impl<C: Collection> Muxer<C> {
    pub fn new(http_origin: HttpOriginFetcher<C>) -> Self {
        todo!()
    }

    pub fn spawn(self) -> (JoinHandle<Self>, Arc<Notify>) {
        todo!()
    }
}
