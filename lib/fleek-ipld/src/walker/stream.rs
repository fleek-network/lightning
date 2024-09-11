use std::pin::Pin;
use std::task::{Context, Poll};

use futures::future::BoxFuture;
use futures::TryFuture;
use tokio_stream::Stream;

use super::downloader::Downloader;
use crate::decoder::data_codec::Decoder;
use crate::decoder::fs::{DirItem, DocId, IpldItem, Link};
use crate::decoder::reader::IpldReader;
use crate::errors::IpldError;

pub struct IpldStream<D, C> {
    downloader: D,
    reader: IpldReader<C>,
    stack: Vec<(Option<DirItem>, Vec<Link>, usize)>,
    pending_future: Option<BoxFuture<'static, Result<Option<IpldItem>, IpldError>>>,
}

impl<D, C> IpldStream<D, C>
where
    D: Downloader + Clone + Send + Sync + 'static,
    C: Decoder + Clone + Send + Sync + 'static,
{
    pub fn new(downloader: D, reader: IpldReader<C>, id: DocId) -> Self {
        Self {
            downloader,
            reader,
            stack: vec![(None, vec![id.into()], 0)],
            pending_future: None,
        }
    }
}

/// Implementing Stream for IpldStream in order to traverse all the links associated to a given Cid
impl<D, C> Stream for IpldStream<D, C>
where
    D: Downloader + Clone + Unpin + Send + Sync + 'static,
    C: Decoder + Clone + Unpin + Send + Sync + 'static,
{
    type Item = Result<IpldItem, IpldError>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();

        if let Some(fut) = this.pending_future.as_mut() {
            match Pin::new(fut).try_poll(cx) {
                Poll::Ready(result) => {
                    this.pending_future = None;
                    match result {
                        Ok(Some(ref i @ IpldItem::Dir(ref item))) => {
                            let links = item.links().clone();
                            this.stack.push((Some(item.clone()), links, 0));
                            return Poll::Ready(Some(Ok(i.clone())));
                        },
                        Ok(Some(item @ IpldItem::File(_))) => {
                            return Poll::Ready(Some(Ok(item)));
                        },
                        Ok(Some(chunk @ IpldItem::ChunkedFile(_))) => {
                            return Poll::Ready(Some(Ok(chunk)));
                        },
                        Ok(None) => {
                            return Poll::Ready(None);
                        },
                        Err(e) => {
                            return Poll::Ready(Some(Err(e)));
                        },
                    }
                },
                Poll::Pending => {
                    return Poll::Pending;
                },
            }
        }

        while let Some((current_dir, current_links, current_index)) = this.stack.last_mut() {
            if *current_index < current_links.len() {
                let link = &current_links[*current_index];
                *current_index += 1;
                let mut reader = this.reader.clone();
                let downloader = this.downloader.clone();
                let cid = DocId::from_link(link, current_dir.as_ref());
                this.pending_future = Some(Box::pin(async move {
                    let bytes = downloader.download(cid.cid()).await?;
                    let item = reader.read(cid, bytes).await?;
                    Ok(Some(item))
                }));
                cx.waker().wake_by_ref();
                return Poll::Pending;
            } else {
                this.stack.pop();
            }
        }

        Poll::Ready(None)
    }
}
