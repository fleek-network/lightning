use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use futures::{StreamExt, TryStreamExt};
use ipld_core::cid::Cid;
use tokio::sync::{mpsc, Mutex};

use crate::decoder::data_codec::Decoder;
use crate::decoder::fs::{DocId, IpldItem};
use crate::decoder::reader::IpldReader;
use crate::errors::IpldError;
use crate::walker::data::{Item, Metadata};
use crate::walker::downloader::Downloader;

#[async_trait]
pub trait IpldItemProcessor {
    async fn on_item(&self, item: Item) -> Result<(), IpldError>;
}

pub struct NOOPProcessor;

#[async_trait]
impl IpldItemProcessor for NOOPProcessor {
    async fn on_item(&self, _item: Item) -> Result<(), IpldError> {
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq)]
enum StreamState {
    Running,
    Paused,
    Stopped,
}

#[derive(Clone)]
pub struct Control {
    inner: mpsc::Sender<StreamState>,
}

impl Control {
    async fn send(&self, state: StreamState) -> Result<(), IpldError> {
        self.inner
            .send(state)
            .await
            .map_err(|e| IpldError::ControlError(e.to_string()))
    }

    pub async fn pause(&self) -> Result<(), IpldError> {
        self.send(StreamState::Paused).await
    }

    pub async fn resume(&self) -> Result<(), IpldError> {
        self.send(StreamState::Running).await
    }

    pub async fn stop(&self) -> Result<(), IpldError> {
        self.send(StreamState::Stopped).await
    }
}

#[derive(Clone)]
pub struct IpldBulkProcessor<C, D> {
    reader: IpldReader<C>,
    downloader: Arc<D>,
    state: Arc<AtomicBool>,
    control_rx: Arc<Mutex<mpsc::Receiver<StreamState>>>,
    control_tx: mpsc::Sender<StreamState>,
    stop: Arc<AtomicBool>,
}

impl<C, D> IpldBulkProcessor<C, D>
where
    C: Decoder + Clone + Send + Sync + 'static,
    D: Downloader + Clone + Send + Sync + 'static,
{
    pub fn new(reader: IpldReader<C>, downloader: D) -> Self {
        let (control_tx, control_rx) = mpsc::channel(100);
        let state = Arc::new(AtomicBool::new(true));
        let stop = Arc::new(AtomicBool::new(false));
        IpldBulkProcessor {
            reader,
            downloader: Arc::new(downloader),
            state,
            control_rx: Arc::new(Mutex::new(control_rx)),
            control_tx,
            stop,
        }
    }

    pub fn control(&self) -> Control {
        Control {
            inner: self.control_tx.clone(),
        }
    }

    pub async fn download<F>(self, cid: Cid, processor: F) -> Result<(), IpldError>
    where
        F: IpldItemProcessor + Send + Sync + Clone + 'static,
    {
        self.spawn_control_watcher().await;
        self.process(cid, Metadata::default(), processor).await
    }

    async fn spawn_control_watcher(&self) {
        let state = self.state.clone();
        let control_rx = self.control_rx.clone();
        let stop = self.stop.clone();

        tokio::spawn(async move {
            while let Some(command) = control_rx.lock().await.recv().await {
                match command {
                    StreamState::Running => state.store(true, Ordering::Release),
                    StreamState::Paused => state.store(false, Ordering::Release),
                    StreamState::Stopped => {
                        stop.store(true, Ordering::Release);
                        break;
                    },
                }
            }
        });
    }

    async fn process<F>(&self, cid: Cid, metadata: Metadata, processor: F) -> Result<(), IpldError>
    where
        F: IpldItemProcessor + Send + Sync + Clone + 'static,
    {
        while !self.state.load(Ordering::Acquire) {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }

        if self.stop.load(Ordering::Acquire) {
            return Ok(());
        }

        let item = self
            .download_item(cid, metadata.clone(), processor.clone())
            .await?;
        self.process_item(item, processor).await
    }

    pub(crate) async fn process_item<F>(
        &self,
        item: IpldItem,
        processor: F,
    ) -> Result<(), IpldError>
    where
        F: IpldItemProcessor + Send + Sync + Clone + 'static,
    {
        let parent_item: DocId = item.clone().into();
        let links = item.links();
        if links.is_empty() {
            return Ok(());
        }
        let futures = links.clone().into_iter().enumerate().map(|(i, link)| {
            let parent_path = parent_item.path().clone();
            let metadata = Metadata::new(i, links.len(), &link, parent_path);
            self.process(*link.cid(), metadata, processor.clone())
        });
        tokio_stream::iter(futures)
            .buffer_unordered(5)
            .try_for_each_concurrent(5, |_| async { Ok(()) })
            .await
    }

    pub async fn download_item<F>(
        &self,
        cid: Cid,
        metadata: Metadata,
        processor: F,
    ) -> Result<IpldItem, IpldError>
    where
        F: IpldItemProcessor + Send + Sync + Clone + 'static,
    {
        let response = self.downloader.download(&cid).await?;
        let mut reader = self.reader.clone();
        let mut result = reader.read(cid, response.fuse()).await?;
        result.merge_path(metadata.parent_path(), metadata.name());
        let item = Item::from_ipld(&result, metadata);
        processor.on_item(item).await?;
        Ok(result)
    }
}
