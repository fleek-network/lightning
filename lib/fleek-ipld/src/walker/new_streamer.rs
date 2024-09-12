use std::collections::VecDeque;
use std::path::PathBuf;
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
    dir_entries: Vec<Link>,
    cache_items: Arc<RwLock<Vec<IpldItem>>>,
}

impl StreamState {
    fn current_cid(&mut self, cid: Cid) {
        self.current_cid = Some(cid);
        self.initial_cid = true;
    }

    fn add_dir_entry(&mut self, links: Vec<Link>) {
        self.dir_entries = links;
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
        if let Some(cid) = self.state.current_cid {
            let item = self.download_item(cid, Metadata::default()).await.map(Some);
            self.state.current_cid = None;
            return item;
        }
        if !self.state.dir_entries.is_empty() {
            self.next_items(10).await?;
        }
        Ok(self.state.get_next_dir_entry().await)
    }

    pub async fn explore_dir(&mut self, item: DirItem) -> Result<(), IpldError> {
        let links = item.links();
        if links.is_empty() {
            return Ok(());
        }
        self.state.add_dir_entry(links.clone());
        Ok(())
    }

    async fn next_items(&mut self, num_items: usize) -> Result<(), IpldError> {
        let num_items = num_items.min(self.state.dir_entries.len());
        let iter = self
            .state
            .dir_entries
            .drain(..num_items)
            .collect::<Vec<_>>();
        let futures = iter.into_iter().map(|link| {
            let self_clone = self.clone();
            async move {
                let metadata = Metadata::new(0, 0, &link, PathBuf::new());
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
