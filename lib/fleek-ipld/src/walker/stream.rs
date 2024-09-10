use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;

use async_trait::async_trait;
use bytes::Bytes;
use futures::{Stream, StreamExt, TryStreamExt};
use ipld_core::cid::Cid;
use typed_builder::TypedBuilder;
use url::Url;

use crate::decoder::data_codec::Decoder;
use crate::decoder::fs::{DocId, IpldItem};
use crate::decoder::reader::IpldReader;
use crate::errors::IpldError;

#[derive(Clone, Debug)]
pub struct Dir {
    pub id: DocId,
    pub name: Option<String>,
}

#[derive(Clone, Debug)]
pub struct Chunked {
    pub index: u64,
    pub total: u64,
}

#[derive(Clone)]
pub struct ItemFile {
    pub id: DocId,
    pub name: Option<String>,
    pub size: Option<u64>,
    pub data: Bytes,
    pub chunked: Option<Chunked>,
}

impl std::fmt::Debug for ItemFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ItemFile")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("size", &self.size)
            .field("chunked", &self.chunked)
            .field("data-length", &self.data.len())
            .finish()
    }
}

#[derive(Clone, Debug)]
pub enum Item {
    Directory(Dir),
    File(ItemFile),
    Skip,
}

#[async_trait]
pub trait IpldItemProcessor {
    async fn on_item(&self, item: &Item) -> Result<(), IpldError>;
}

pub struct NOOPProcessor;

#[async_trait]
impl IpldItemProcessor for NOOPProcessor {
    async fn on_item(&self, _item: &Item) -> Result<(), IpldError> {
        Ok(())
    }
}

pub type Response = Pin<Box<dyn Stream<Item = Result<Bytes, IpldError>> + Send + Sync + 'static>>;

pub trait Downloader {
    fn download(
        &self,
        cid: &Cid,
    ) -> impl std::future::Future<Output = Result<Response, IpldError>> + Send;
}

#[derive(Clone)]
pub struct ReqwestDownloader {
    url: Url,
}

impl ReqwestDownloader {
    pub fn new(url: &str) -> Self {
        let ipfs_url = Url::parse(url).unwrap_or_else(|_| panic!("Invalid IPFS URL {}", url));
        ReqwestDownloader { url: ipfs_url }
    }
}

impl Downloader for ReqwestDownloader {
    async fn download(&self, cid: &Cid) -> Result<Response, IpldError> {
        let url = self.url.join(&format!("ipfs/{}?format=raw", cid))?;
        let response = reqwest::get(url).await?;
        let stream = response.bytes_stream().map_err(Into::into);
        Ok(Box::pin(stream))
    }
}

#[derive(Clone)]
pub struct IpldStream<C, D, P> {
    reader: IpldReader<C>,
    downloader: Arc<D>,
    processor: Arc<P>,
}

#[derive(Default, Clone, TypedBuilder)]
struct Metadata {
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

impl<C, D, P> IpldStream<C, D, P>
where
    C: Decoder + Clone + Send + Sync + 'static,
    D: Downloader + Clone + Send + Sync + 'static,
    P: IpldItemProcessor + Clone + Send + Sync + 'static,
{
    pub fn new(reader: IpldReader<C>, downloader: D, processor: P) -> Self {
        IpldStream {
            reader,
            downloader: Arc::new(downloader),
            processor: Arc::new(processor),
        }
    }

    pub async fn download(self, cid: Cid) -> Result<(), IpldError> {
        self.process(cid, Metadata::default()).await
    }

    async fn process(&self, cid: Cid, metadata: Metadata) -> Result<(), IpldError> {
        let item = self.download_item(cid, metadata.clone()).await?;
        self.process_item(item).await
    }

    async fn process_item(&self, item: IpldItem) -> Result<(), IpldError> {
        let parent_item: DocId = item.clone().into();
        let links = item.links();
        if links.is_empty() {
            return Ok(());
        }
        let futures = links.clone().into_iter().enumerate().map(|(i, link)| {
            let parent_path = parent_item.path().clone();
            let mut metadata = Metadata::builder().parent_path(parent_path.clone()).build();
            if !item.is_dir() {
                metadata = Metadata::builder()
                    .parent_path(parent_path)
                    .size(*link.size())
                    .name(link.name().clone())
                    .index(i as u64)
                    .total(links.len() as u64)
                    .build();
            }
            self.process(*link.cid(), metadata)
        });
        tokio_stream::iter(futures)
            .buffer_unordered(20)
            .collect::<Vec<Result<(), IpldError>>>()
            .await
            .into_iter()
            .collect::<Result<(), IpldError>>()?;

        //let mut set_links: JoinSet<Result<(), IpldError>> = JoinSet::new();
        //for (i, link) in links.clone().into_iter().enumerate() {
        //    let parent_path = parent_item.path().clone();
        //    let mut metadata = Metadata::builder().parent_path(parent_path.clone()).build();
        //    if !item.is_dir() {
        //        metadata = Metadata::builder()
        //            .parent_path(parent_path)
        //            .size(*link.size())
        //            .name(link.name().clone())
        //            .index(i as u64)
        //            .total(links.len() as u64)
        //            .build();
        //    }
        //    let stream = self.clone();
        //    set_links.spawn(async move { stream.process(link.cid(), metadata).await });
        //}
        //while let Some(res) = set_links.join_next().await {
        //    res??;
        //}
        Ok(())
    }

    async fn download_item(&self, cid: Cid, metadata: Metadata) -> Result<IpldItem, IpldError> {
        let response = self.downloader.download(&cid).await?;
        let mut reader = self.reader.clone();
        let mut item = reader.read(cid, response).await?;
        item.merge_path(&metadata.parent_path);
        let to_item = to_item(&item, metadata);
        self.processor.on_item(&to_item).await?;
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
