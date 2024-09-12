use std::collections::VecDeque;
use std::sync::Arc;

use futures::{StreamExt, TryStreamExt};
use ipld_core::cid::Cid;
use tokio::sync::RwLock;
use typed_builder::TypedBuilder;

use crate::decoder::data_codec::Decoder;
use crate::decoder::fs::{ChunkFileItem, ChunkItem, DirItem, IpldItem, Link};
use crate::decoder::reader::IpldReader;
use crate::errors::IpldError;
use crate::walker::data::Metadata;
use crate::walker::downloader::Downloader;

#[derive(Debug, Clone, Default)]
struct StreamState {
    initial_cid: bool,
    current_cid: Option<Cid>,
    dir_entries: (Option<DirItem>, Vec<Link>),
    cache_items: Arc<RwLock<Vec<IpldItem>>>,
    last_item: Option<IpldItem>,
}

impl StreamState {
    fn current_cid(&mut self, cid: Cid) {
        self.current_cid = Some(cid);
        self.initial_cid = true;
    }

    fn add_dir_entry(&mut self, dir_item: DirItem, links: Vec<Link>) {
        self.dir_entries = (Some(dir_item), links);
    }

    fn get_list_entries(&self) -> Vec<Link> {
        self.dir_entries.1.clone()
    }

    fn get_dir_item(&self) -> Option<DirItem> {
        self.dir_entries.0.clone()
    }

    async fn get_next_dir_entry(&mut self) -> Option<IpldItem> {
        self.cache_items.write().await.pop()
    }
}

#[derive(Clone, TypedBuilder)]
pub struct StreamStep<C, D> {
    reader: IpldReader<C>,
    #[builder(setter(transform = |x: D| Arc::new(x)))]
    downloader: Arc<D>,
    #[builder(default = StreamState::default())]
    state: StreamState,
}

impl<C, D> StreamStep<C, D>
where
    C: Decoder + Clone + Send + Sync + 'static,
    D: Downloader + Clone + Send + Sync + 'static,
{
    pub async fn start<I: Into<Cid>>(&mut self, cid: I) {
        self.state.current_cid(cid.into());
    }

    pub async fn next(&mut self) -> Result<Option<IpldItem>, IpldError> {
        assert!(
            self.state.initial_cid,
            "Stream not started. You must call start(cid) first."
        );
        if let Some(item) = &self.state.last_item {
            if item.is_dir() {
                let dir = item.try_into_dir()?;
                self.explore_dir(dir).await?;
            }
        }
        if let Some(cid) = self.state.current_cid {
            let item = self.download_item(cid, Metadata::default()).await.map(Some);
            self.state.current_cid = None;
            if let Ok(item) = &item {
                self.state.last_item = item.clone();
            }
            return item;
        }
        if !self.state.get_list_entries().is_empty() {
            self.cache_next(10).await?;
        }
        let item = self.state.get_next_dir_entry().await;
        self.state.last_item = item.clone();
        Ok(item)
    }

    pub async fn next_n(&mut self, n: usize) -> Result<Vec<IpldItem>, IpldError> {
        let mut items = Vec::with_capacity(n);
        for _ in 0..n {
            if let Some(item) = self.next().await? {
                items.push(item);
            } else {
                break;
            }
        }
        Ok(items)
    }

    async fn explore_dir(&mut self, item: DirItem) -> Result<(), IpldError> {
        let item_dir = item.clone();
        let links = item.links().to_vec();
        if links.is_empty() {
            return Ok(());
        }
        self.state.add_dir_entry(item_dir, links.clone());
        Ok(())
    }

    async fn cache_next(&mut self, num_items: usize) -> Result<(), IpldError> {
        let num_items = num_items.min(self.state.get_list_entries().len());
        let iter = self
            .state
            .get_list_entries()
            .drain(..num_items)
            .collect::<Vec<_>>();
        let futures = iter.into_iter().map(|link| {
            let self_clone = self.clone();
            let parent_path = self
                .state
                .get_dir_item()
                .map(|x| x.id().path().clone())
                .unwrap_or_default();
            async move {
                let metadata = Metadata::new(0, 0, &link, parent_path);
                let item = self_clone.download_item(*link.cid(), metadata).await?;
                self_clone
                    .state
                    .cache_items
                    .write()
                    .await
                    .push(item.clone());
                Ok::<(), IpldError>(())
            }
        });
        tokio_stream::iter(futures)
            .buffer_unordered(5)
            .try_for_each_concurrent(5, |_| async { Ok(()) })
            .await
    }

    pub async fn new_chunk_file_streamer(&self, item: ChunkFileItem) -> StreamChunkedFile<C, D> {
        StreamChunkedFile::new(self.downloader.clone(), self.reader.clone(), item)
    }

    async fn download_item(&self, cid: Cid, metadata: Metadata) -> Result<IpldItem, IpldError> {
        let response = self.downloader.download(&cid).await?;
        let mut reader = self.reader.clone();
        let mut result = reader.read(cid, response.fuse()).await?;
        result.merge_path(metadata.parent_path(), metadata.name());
        Ok(result)
    }
}

pub struct StreamChunkedFile<C, D> {
    reader: IpldReader<C>,
    downloader: Arc<D>,
    item: ChunkFileItem,
    pending_chunks: VecDeque<(usize, Cid)>,
}

impl<C, D> StreamChunkedFile<C, D>
where
    C: Decoder + Clone + Send + Sync + 'static,
    D: Downloader + Clone + Send + Sync + 'static,
{
    pub fn new(downloader: Arc<D>, reader: IpldReader<C>, item: ChunkFileItem) -> Self {
        let vec: Vec<(usize, Cid)> = item
            .chunks()
            .iter()
            .enumerate()
            .map(|(i, c)| (i, *c.cid()))
            .collect();
        let pending_chunks: VecDeque<(usize, Cid)> = VecDeque::from(vec);
        Self {
            reader,
            downloader,
            item,
            pending_chunks,
        }
    }

    pub async fn next_chunk(&mut self) -> Result<Option<ChunkItem>, IpldError> {
        if let Some((i, element)) = self.pending_chunks.pop_front() {
            let response = self.downloader.download(&element).await?;
            let mut reader = self.reader.clone();
            let mut chunk = reader.read(element, response.fuse()).await?;
            chunk.merge_path(self.item.id().path(), None);
            let chunk = chunk.try_into_chunk(i)?;
            Ok(Some(chunk))
        } else {
            Ok(None)
        }
    }
}
