use std::future::Future;
use std::mem;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use tokio::fs::File;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncSeekExt, BufReader};
use tokio::sync::RwLock;
use tokio_stream::Stream;

use crate::bucket::errors;
use crate::entry::{BorrowedEntry, BorrowedLink};
use crate::hasher::dir_hasher::B3_DIR_IS_SYM_LINK;
use crate::on_future;

/// State machine states for iterating through directory entries
enum State {
    /// Initial state - need to seek to entry position
    ReadPosition,
    /// Reading the entry type flag
    ReadFlag,
    /// Reading the entry name
    ReadEntryName,
    /// Reading content hash for regular entries
    ReadContent,
    /// Reading symlink path for symlink entries
    ReadPath,
    /// End of file reached
    Eof,
    WaitingReadPosition(Pin<Box<dyn Future<Output = Result<u64, std::io::Error>>>>),
    WaitingReadFlag(Pin<Box<dyn Future<Output = Result<u8, std::io::Error>>>>),
    WaitingReadEntryName(Pin<Box<dyn Future<Output = Result<Vec<u8>, std::io::Error>>>>),
    WaitingReadContent(Pin<Box<dyn Future<Output = Result<[u8; 32], std::io::Error>>>>),
    WaitingReadPath(Pin<Box<dyn Future<Output = Result<Vec<u8>, std::io::Error>>>>),
}

/// Iterator over entries in a B3 directory
pub struct DirEntriesIter<'a> {
    /// Buffered reader for the directory file
    reader: Arc<RwLock<BufReader<File>>>,
    /// Current position in the file
    position: u64,
    /// Current state of the iterator state machine
    state: State,
    /// Buffer for entry name being read
    entry_name: Vec<u8>,
    /// Entry type flag
    flag: u8,
    /// Phantom data for lifetime 'a
    _marker: std::marker::PhantomData<&'a ()>,
}

impl<'a> DirEntriesIter<'a> {
    /// Creates a new directory entries iterator starting at the given position
    pub async fn new(reader: File, position_start_entries: u64) -> Result<Self, errors::ReadError> {
        let mut reader = BufReader::new(reader);
        Ok(Self {
            reader: Arc::new(RwLock::new(reader)),
            position: position_start_entries,
            state: State::ReadPosition,
            entry_name: Vec::new(),
            flag: 0,
            _marker: std::marker::PhantomData,
        })
    }
}

impl<'a> Stream for DirEntriesIter<'a> {
    type Item = Result<BorrowedEntry<'a>, errors::ReadError>;

    /// Polls the iterator for the next directory entry
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.as_mut().get_mut();

        match this.state {
            State::Eof => Poll::Ready(None),
            State::ReadPosition => {
                // Reset buffers and seek to next entry position
                this.entry_name.clear();
                this.flag = 0;
                let file = this.reader.clone();
                let pos = this.position;
                let waker = cx.waker().clone();
                let future = async move {
                    let result = file
                        .write()
                        .await
                        .seek(tokio::io::SeekFrom::Start(pos))
                        .await;
                    waker.wake();
                    result
                };
                let future = Box::pin(future);
                this.state = State::WaitingReadPosition(future);
                cx.waker().wake_by_ref();
                Poll::Pending
            },
            State::WaitingReadPosition(ref mut future) => {
                let result = future.as_mut().poll(cx);
                on_future!(this, result, State::Eof, |r| {
                    this.state = State::ReadFlag;
                    cx.waker().wake_by_ref();
                    Poll::Pending
                })
            },
            State::ReadFlag => {
                let file = this.reader.clone();
                let waker = cx.waker().clone();
                let future = async move {
                    let result = file.write().await.read_u8().await;
                    waker.wake();
                    result
                };
                let future = Box::pin(future);
                this.state = State::WaitingReadFlag(future);
                cx.waker().wake_by_ref();
                Poll::Pending
            },
            State::WaitingReadFlag(ref mut future) => {
                let result = future.as_mut().poll(cx);
                on_future!(this, result, State::Eof, |r| {
                    this.state = State::ReadEntryName;
                    this.flag = r;
                    cx.waker().wake_by_ref();
                    Poll::Pending
                })
            },
            State::ReadEntryName => {
                // Read entry name until null terminator
                let file = this.reader.clone();
                let waker = cx.waker().clone();
                let future = async move {
                    let mut buffer = Vec::new();
                    file.write().await.read_until(0x00, &mut buffer).await?;
                    waker.wake();
                    Ok(buffer)
                };
                let future = Box::pin(future);
                this.state = State::WaitingReadEntryName(future);
                cx.waker().wake_by_ref();
                Poll::Pending
            },
            State::WaitingReadEntryName(ref mut future) => {
                let result = future.as_mut().poll(cx);
                let state = if this.flag & B3_DIR_IS_SYM_LINK != 0 {
                    State::ReadPath
                } else {
                    State::ReadContent
                };
                on_future!(this, result, State::Eof, |r| {
                    this.state = state;
                    this.entry_name = r;
                    this.entry_name.pop();
                    cx.waker().wake_by_ref();
                    Poll::Pending
                })
            },
            State::ReadContent => {
                // Read content hash for regular entries
                let file = this.reader.clone();
                let waker = cx.waker().clone();
                let future = async move {
                    let mut buffer = [0u8; 32];
                    file.write().await.read_exact(&mut buffer).await?;
                    waker.wake();
                    Ok(buffer)
                };
                let future = Box::pin(future);
                this.state = State::WaitingReadContent(future);
                cx.waker().wake_by_ref();
                Poll::Pending
            },
            State::WaitingReadContent(ref mut fut) => {
                let result = fut.as_mut().poll(cx);
                on_future!(this, result, State::Eof, |buffer_content: [u8; 32]| {
                    this.state = State::ReadPosition;
                    this.position += this.entry_name.len() as u64 + buffer_content.len() as u64 + 2;
                    let static_slice: &'static [u8; 32] =
                        unsafe { mem::transmute_copy(&buffer_content) };
                    Poll::Ready(Some(Ok(BorrowedEntry {
                        name: Box::leak(this.entry_name.clone().into_boxed_slice()),
                        link: BorrowedLink::Content(static_slice),
                    })))
                })
            },
            State::ReadPath => {
                // Read symlink path until null terminator
                let file = this.reader.clone();
                let waker = cx.waker().clone();
                let future = async move {
                    let mut buffer = Vec::new();
                    file.write().await.read_until(0x00, &mut buffer).await?;
                    waker.wake();
                    Ok(buffer)
                };
                let future = Box::pin(future);
                this.state = State::WaitingReadPath(future);
                cx.waker().wake_by_ref();
                Poll::Pending
            },
            State::WaitingReadPath(ref mut future) => {
                let result = future.as_mut().poll(cx);
                on_future!(this, result, State::Eof, |buffer_link: Vec<u8>| {
                    this.state = State::ReadPosition;
                    this.position += this.entry_name.len() as u64 + buffer_link.len() as u64 + 2;
                    let content = buffer_link[..buffer_link.len() - 1]
                        .to_vec()
                        .into_boxed_slice();
                    let static_slice: &'static [u8] = Box::leak(content);
                    Poll::Ready(Some(Ok(BorrowedEntry {
                        name: Box::leak(this.entry_name.clone().into_boxed_slice()),
                        link: BorrowedLink::Path(static_slice),
                    })))
                })
            },
        }
    }
}
