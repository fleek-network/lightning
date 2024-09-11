use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use futures::{StreamExt, TryStreamExt};
use ipld_core::cid::Cid;
use tokio::sync::{mpsc, Mutex};

use super::processor::{IpldItemProcessor, IpldStream, Metadata};
use crate::decoder::data_codec::Decoder;
use crate::decoder::fs::{DocId, IpldItem};
use crate::decoder::reader::IpldReader;
use crate::errors::IpldError;
use crate::walker::downloader::Downloader;

#[derive(Debug, Clone, PartialEq)]
pub enum StreamState {
    Running,
    Paused,
    Stopped,
}

pub struct ControlledIpldStream<C, D> {
    inner: IpldStream<C, D>,
    state: Arc<AtomicBool>,
    control_rx: Arc<Mutex<mpsc::Receiver<StreamState>>>,
    control_tx: mpsc::Sender<StreamState>,
    stop: Arc<AtomicBool>,
}

impl<C, D> ControlledIpldStream<C, D>
where
    C: Decoder + Clone + Send + Sync + 'static,
    D: Downloader + Clone + Send + Sync + 'static,
{
    pub fn new(reader: IpldReader<C>, downloader: D) -> Self {
        let (control_tx, control_rx) = mpsc::channel(100);
        ControlledIpldStream {
            inner: IpldStream::new(reader, downloader),
            state: Arc::new(AtomicBool::new(true)),
            control_rx: Arc::new(Mutex::new(control_rx)),
            control_tx,
            stop: Arc::new(AtomicBool::new(false)),
        }
    }

    pub async fn download<F>(&mut self, cid: Cid, processor: F) -> Result<(), IpldError>
    where
        F: IpldItemProcessor + Clone + Send + Sync + 'static,
    {
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

        self.controlled_process(cid, Metadata::default(), processor)
            .await
    }

    async fn controlled_process<F>(
        &self,
        cid: Cid,
        metadata: Metadata,
        processor: F,
    ) -> Result<(), IpldError>
    where
        F: IpldItemProcessor + Clone + Send + Sync + 'static,
    {
        while !self.state.load(Ordering::Acquire) {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }

        if self.stop.load(Ordering::Acquire) {
            return Ok(());
        }

        let item = self
            .inner
            .download_item(cid, metadata.clone(), processor.clone())
            .await?;
        self.controlled_process_item(item, processor).await
    }

    async fn controlled_process_item<F>(
        &self,
        item: IpldItem,
        processor: F,
    ) -> Result<(), IpldError>
    where
        F: IpldItemProcessor + Clone + Send + Sync + 'static,
    {
        let parent_item: DocId = item.clone().into();
        let links = item.links();
        if links.is_empty() {
            return Ok(());
        }

        let futures = links.clone().into_iter().enumerate().map(|(i, link)| {
            let parent_path = parent_item.path().clone();
            let metadata = Metadata::new(i, links.len(), &link, parent_path);
            self.controlled_process(*link.cid(), metadata, processor.clone())
        });

        tokio_stream::iter(futures)
            .buffer_unordered(5)
            .try_for_each_concurrent(5, |_| async { Ok(()) })
            .await
    }

    pub fn control(&self) -> mpsc::Sender<StreamState> {
        self.control_tx.clone()
    }
}
