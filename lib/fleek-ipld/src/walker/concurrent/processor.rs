use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use futures::StreamExt;
use ipld_core::cid::Cid;
use typed_builder::TypedBuilder;

use crate::decoder::data_codec::Decoder;
use crate::decoder::fs::{DocId, IpldItem};
use crate::decoder::reader::IpldReader;
use crate::errors::IpldError;
use crate::walker::data::{Chunked, Dir, Item, ItemFile};
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

#[derive(Default, Debug, Clone, TypedBuilder)]
pub struct Metadata {
    #[builder(default = PathBuf::new())]
    parent_path: PathBuf,
    #[builder(default)]
    size: Option<u64>,
    #[builder(default)]
    name: Option<String>,
    #[builder(default, setter(into, strip_option))]
    index: Option<u64>,
    #[builder(default, setter(into, strip_option))]
    total: Option<u64>,
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

    async fn process_item<F>(&self, item: IpldItem, processor: F) -> Result<(), IpldError>
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
            let metadata = Metadata::builder()
                .parent_path(parent_path)
                .size(*link.size())
                .name(link.name().clone())
                .index(i as u64)
                .total(links.len() as u64)
                .build();
            self.process(*link.cid(), metadata, processor.clone())
        });
        tokio_stream::iter(futures)
            .buffer_unordered(20)
            .collect::<Vec<Result<(), IpldError>>>()
            .await
            .into_iter()
            .collect::<Result<(), IpldError>>()
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
        let mut item = reader.read(cid, response).await?;
        item.merge_path(&metadata.parent_path, metadata.name.as_deref());
        processor.on_item(to_item(&item, metadata)).await?;
        Ok(item)
    }
}

fn to_item(
    item: &IpldItem,
    Metadata {
        size,
        name,
        index,
        total,
        ..
    }: Metadata,
) -> Item {
    match item {
        IpldItem::Dir(dir) => Item::Directory(Dir {
            id: dir.id().clone(),
            name,
        }),
        IpldItem::File(file) => Item::File(ItemFile {
            id: file.id().clone(),
            name,
            size,
            data: file.data().clone(),
            chunked: index.and_then(|index| total.map(|total| Chunked { index, total })),
        }),
        IpldItem::ChunkedFile(_) => Item::Skip,
    }
}
