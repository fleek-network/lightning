use std::sync::Arc;

use async_trait::async_trait;
use futures::{StreamExt, TryStreamExt};
use ipld_core::cid::Cid;

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

#[derive(Clone)]
pub struct IpldStream<C, D> {
    reader: IpldReader<C>,
    downloader: Arc<D>,
}

impl<C, D> IpldStream<C, D>
where
    C: Decoder + Clone + Send + Sync + 'static,
    D: Downloader + Clone + Send + Sync + 'static,
{
    pub fn new(reader: IpldReader<C>, downloader: D) -> Self {
        IpldStream {
            reader,
            downloader: Arc::new(downloader),
        }
    }

    pub async fn download<F>(self, cid: Cid, processor: F) -> Result<(), IpldError>
    where
        F: IpldItemProcessor + Send + Sync + Clone + 'static,
    {
        self.process(cid, Metadata::default(), processor).await
    }

    async fn process<F>(&self, cid: Cid, metadata: Metadata, processor: F) -> Result<(), IpldError>
    where
        F: IpldItemProcessor + Send + Sync + Clone + 'static,
    {
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
