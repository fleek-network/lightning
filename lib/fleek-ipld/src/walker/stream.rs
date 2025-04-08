//! This module contains the implementation of the Walker with a Stream fashion approach.
use std::collections::VecDeque;
use std::sync::Arc;

use futures::StreamExt;
use ipld_core::cid::Cid;
use tokio::task::JoinSet;
use typed_builder::TypedBuilder;

use crate::decoder::data_codec::Decoder;
use crate::decoder::fs::{ChunkFileItem, ChunkItem, DirItem, DocId, IpldItem, Link};
use crate::decoder::reader::IpldReader;
use crate::errors::IpldError;
use crate::walker::data::Metadata;
use crate::walker::downloader::Downloader;

#[derive(Debug, Clone, Default)]
struct StreamState {
    initial_cid: bool,
    current_cid: Option<Cid>,
    dir_entries: Vec<(DocId, Link, Option<DocId>)>,
    cache_items: Vec<(IpldItem, Option<DocId>)>,
    last_item: Option<(IpldItem, Option<DocId>)>,
}

impl StreamState {
    fn current_cid(&mut self, cid: Cid) {
        self.current_cid = Some(cid);
        self.initial_cid = true;
    }

    fn add_dir_entry(&mut self, dir_item: DocId, parent: Option<DocId>, links: Vec<Link>) {
        self.dir_entries.extend(
            links
                .iter()
                .map(|x| (dir_item.clone(), x.clone(), parent.clone())),
        );
    }

    fn get_list_entries(&self) -> &Vec<(DocId, Link, Option<DocId>)> {
        &self.dir_entries
    }

    fn get_next_dir_entry(&mut self) -> Option<(IpldItem, Option<DocId>)> {
        self.cache_items.pop()
    }
}

/// The `IpldStream` struct is used to stream IPLD data from IPFS.
#[derive(Clone, TypedBuilder)]
pub struct IpldStream<C, D> {
    reader: IpldReader<C>,
    #[builder(setter(transform = |x: D| Arc::new(x)))]
    downloader: Arc<D>,
    #[builder(default = StreamState::default())]
    state: StreamState,
}

impl<C, D> IpldStream<C, D>
where
    C: Decoder + Clone + Send + Sync + 'static,
    D: Downloader + Clone + Send + Sync + 'static,
{
    /// Start the stream with the given CID.
    ///
    /// This function must be called before calling `next`.
    pub async fn start<I: Into<Cid>>(&mut self, cid: I) {
        self.state.current_cid(cid.into());
    }

    /// Get the next item from the stream.
    ///
    /// This function will return the next item from the stream. If there are no more items, it will
    /// return `Ok(None)`.
    ///
    /// **Note**: You must call `start` before calling this function.
    pub async fn next(&mut self) -> Result<Option<(IpldItem, Option<DocId>)>, IpldError> {
        // Not allow calling next if start has not been called
        assert!(
            self.state.initial_cid,
            "Stream not started. You must call start(cid) first."
        );

        // If there was an item in the previous iteration, explore it if it is a directory
        // in order to store the links for the next iteration
        if let Some(item) = &self.state.last_item {
            if item.0.is_dir() {
                let dir = item.0.try_into_dir()?;
                self.explore_dir(dir).await?;
            }
            self.state.last_item = None;
        }

        // If there is a current_cid which means first iteration, download it and return the item.
        // Parent is always none for the root item.
        if let Some(cid) = self.state.current_cid {
            let item = self
                .download_item(cid, Metadata::default(), None)
                .await
                .map(Some);
            self.state.current_cid = None;
            if let Ok(item) = &item {
                self.state.last_item = item.clone();
            }
            return item;
        }

        // If there are items to be explored, download 10 of them in parallel and cache them
        if !self.state.get_list_entries().is_empty() {
            self.cache_next(10).await?;
        }

        // Return the next item from the cache
        let item = self.state.get_next_dir_entry();
        self.state.last_item = item.clone();
        Ok(item)
    }

    /// Get the next `n` items from the stream, instead of just one.
    pub async fn next_n(&mut self, n: usize) -> Result<Vec<(IpldItem, Option<DocId>)>, IpldError> {
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
        let parent_id = item.id().clone();
        let links = item.links();
        if links.is_empty() {
            return Ok(());
        }
        self.state
            .add_dir_entry(item.id().clone(), Some(parent_id), links.clone());
        Ok(())
    }

    async fn cache_next(&mut self, num_items: usize) -> Result<(), IpldError> {
        let num_items = num_items.min(self.state.get_list_entries().len());
        let iter = self
            .state
            .dir_entries
            .drain(..num_items)
            .collect::<Vec<_>>();
        let mut set = JoinSet::new();
        for (id, link, parent) in iter {
            let self_clone = self.clone();
            set.spawn(async move {
                let metadata = Metadata::new(0, 0, &link, id.path().clone());
                self_clone
                    .download_item(*link.cid(), metadata, parent)
                    .await
            });
        }
        let mut vec = Vec::with_capacity(num_items);
        while let Some(result) = set.join_next().await {
            let item = result??;
            vec.push(item);
        }
        self.state.cache_items.extend(vec);
        Ok(())
    }

    /// When you receive a `ChunkedFile` item, from the caller, this is a signal that the file is
    /// split into multiple chunks. `ChunkedFile` DOES NOT contain the actual data, because it will
    /// be more efficient to download the chunks lazyly as they are needed. This function will
    /// return a stream that you can use to download the chunks.
    pub async fn new_chunk_file_streamer(&self, item: ChunkFileItem) -> StreamChunkedFile<C, D> {
        StreamChunkedFile::new(self.downloader.clone(), self.reader.clone(), item)
    }

    async fn download_item(
        &self,
        cid: Cid,
        metadata: Metadata,
        parent: Option<DocId>,
    ) -> Result<(IpldItem, Option<DocId>), IpldError> {
        let response = self.downloader.download(&cid).await?;
        let mut reader = self.reader.clone();
        let mut result = reader.read(cid, response.fuse()).await?;
        result.merge_path(metadata.parent_path(), metadata.name());
        Ok((result, parent))
    }
}

/// The `StreamChunkedFile` struct is used to stream chunks of a `ChunkedFile` item.
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
    pub(crate) fn new(downloader: Arc<D>, reader: IpldReader<C>, item: ChunkFileItem) -> Self {
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

    /// Get the next chunk from the stream.
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
