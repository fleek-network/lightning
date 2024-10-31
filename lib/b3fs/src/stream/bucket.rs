use std::future::{self, Future};
use std::io::SeekFrom;
use std::mem;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use tokio::fs::File;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncSeekExt, BufReader};
use tokio::sync::RwLock;
use tokio_stream::Stream;

use crate::bucket::dir::reader::B3Dir;
use crate::bucket::dir::HeaderPositions;
use crate::bucket::file::reader::B3File;
use crate::bucket::{
    self,
    errors,
    Bucket,
    HEADER_TYPE_DIR,
    POSITION_START_HASHES,
    POSITION_START_NUM_ENTRIES,
};
use crate::entry::{BorrowedEntry, OwnedEntry};
use crate::hasher::dir_hasher::B3_DIR_IS_SYM_LINK;
use crate::on_future;
use crate::utils::tree_index;

pub struct BucketStream;

pub struct Block(pub Vec<u8>);

pub enum DirItem {
    File(PathBuf),
    Dir(PathBuf),
    Symblink(Vec<u8>),
}

pub enum BucketItem {
    File(Block),
    Dir(DirItem),
}

pub type BucketStreamer =
    Result<Box<dyn Stream<Item = Result<BucketItem, errors::ReadError>>>, errors::ReadError>;

impl BucketStream {
    pub async fn create(bucket: &Bucket, root_hash: &[u8; 32]) -> BucketStreamer {
        let head = bucket.get_header_path(root_hash);
        let mut file = File::open(head).await?;
        let ty = file.read_u8().await?;
        let stream: Box<dyn Stream<Item = Result<BucketItem, errors::ReadError>>> =
            if ty == HEADER_TYPE_DIR {
                Box::new(DirStream::new(file, bucket.to_owned()).await?)
            } else {
                Box::new(FileStream::new(file, bucket.to_owned()).await?)
            };
        Ok(stream)
    }
}

enum FileState {
    SeekPositionHash,
    WaitingPositionHash(Pin<Box<dyn Future<Output = Result<u64, std::io::Error>>>>),
    ReadHash,
    WaitingHash(Pin<Box<dyn Future<Output = std::io::Result<[u8; 32]>>>>),
    ReadContentBlock,
    WaitingContentBlock(Pin<Box<dyn Future<Output = std::io::Result<Vec<u8>>>>>),
    Done,
}

pub struct FileStream {
    bucket: Bucket,
    file: Arc<RwLock<File>>,
    state: FileState,
    current_block: u32,
    num_blocks: u32,
    current_hash: [u8; 32],
}

impl FileStream {
    pub async fn new(mut file: File, bucket: Bucket) -> Result<Self, errors::ReadError> {
        file.seek(SeekFrom::Start(POSITION_START_NUM_ENTRIES as u64))
            .await?;
        let num_blocks = file.read_u32_le().await?;
        Ok(Self {
            bucket,
            file: Arc::new(RwLock::new(file)),
            state: FileState::SeekPositionHash,
            num_blocks,
            current_block: 0,
            current_hash: [0u8; 32],
        })
    }
}

impl Stream for FileStream {
    type Item = Result<BucketItem, errors::ReadError>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        if this.current_block >= this.num_blocks {
            this.state = FileState::Done;
        }

        match this.state {
            FileState::SeekPositionHash => {
                let waker = cx.waker().clone();
                let index = tree_index(this.current_block as usize) as u64 * 32
                    + POSITION_START_HASHES as u64;
                let file = this.file.clone();
                let future = async move {
                    let result = file.write().await.seek(SeekFrom::Start(index)).await;
                    waker.wake();
                    result
                };
                let future = Box::pin(future);
                this.state = FileState::WaitingPositionHash(future);
                cx.waker().wake_by_ref();
                Poll::Pending
            },
            FileState::WaitingPositionHash(ref mut fut) => match fut.as_mut().poll(cx) {
                Poll::Ready(Ok(_)) => {
                    this.state = FileState::ReadHash;
                    cx.waker().wake_by_ref();
                    Poll::Pending
                },
                Poll::Pending => Poll::Pending,
                Poll::Ready(Err(e)) => Poll::Ready(Some(Err(e.into()))),
            },
            FileState::ReadHash => {
                let waker = cx.waker().clone();
                let file = this.file.clone();
                let future = async move {
                    let mut current_hash = [0u8; 32];
                    let result = file.write().await.read_exact(&mut current_hash).await?;
                    waker.wake();
                    Ok(current_hash)
                };
                let future = Box::pin(future);
                this.state = FileState::WaitingHash(future);
                cx.waker().wake_by_ref();
                Poll::Pending
            },
            FileState::WaitingHash(ref mut fut) => match fut.as_mut().poll(cx) {
                Poll::Ready(Ok(hash)) => {
                    this.current_hash = hash;
                    this.state = FileState::ReadContentBlock;
                    cx.waker().wake_by_ref();
                    Poll::Pending
                },
                Poll::Pending => Poll::Pending,
                Poll::Ready(Err(e)) => Poll::Ready(Some(Err(e.into()))),
            },
            FileState::ReadContentBlock => {
                let curr_block = this.current_block;
                let waker = cx.waker().clone();
                let curr_hash = this.current_hash;
                let buck_path = this.bucket.get_block_path(curr_block, &curr_hash).clone();
                let future = async move {
                    let result = tokio::fs::read(buck_path).await;
                    waker.wake();
                    result
                };
                let future = Box::pin(future);
                this.state = FileState::WaitingContentBlock(future);
                cx.waker().wake_by_ref();
                Poll::Pending
            },
            FileState::WaitingContentBlock(ref mut fut) => match fut.as_mut().poll(cx) {
                Poll::Ready(Ok(content)) => {
                    this.current_block += 1;
                    this.state = FileState::SeekPositionHash;
                    Poll::Ready(Some(Ok(BucketItem::File(Block(content)))))
                },
                Poll::Pending => Poll::Pending,
                Poll::Ready(Err(e)) => Poll::Ready(Some(Err(e.into()))),
            },
            FileState::Done => Poll::Ready(None),
        }
    }
}

type PinBoxFuture<T> = Pin<Box<dyn Future<Output = std::io::Result<T>>>>;

enum DirState {
    SeekNextPosition,
    ReadFlagEntry,
    ReadNameEntry,
    ReadContentEntry,
    OpenContentEntry,
    ReadPathEntry,
    Eof,
    Done,
    WaitingSeekPosition(PinBoxFuture<u64>),
    WaitingReadFlagEntry(PinBoxFuture<u8>),
    WaitingReadNameEntry(PinBoxFuture<Vec<u8>>),
    WaitingContentEntry(PinBoxFuture<[u8; 32]>),
    WaitingOpenContentEntry(PinBoxFuture<(PathBuf, u8)>),
    WaitingReadPathEntry(PinBoxFuture<Vec<u8>>),
}

pub struct DirStream {
    state: DirState,
    file: Arc<RwLock<BufReader<File>>>,
    num_entries: u32,
    bucket: Bucket,
    current_entry: Option<OwnedEntry>,
    count_entry: u32,
    current_position: u64,
    positions: HeaderPositions,
    flag: u8,
    entry_name: Vec<u8>,
    current_buffer_content: [u8; 32],
    current_buffer_path: Vec<u8>,
}

impl DirStream {
    pub async fn new(file: File, bucket: Bucket) -> Result<Self, errors::ReadError> {
        let mut buf_file = BufReader::new(file);
        buf_file
            .seek(SeekFrom::Start(POSITION_START_NUM_ENTRIES as u64))
            .await?;
        let num_entries = buf_file.read_u32_le().await?;
        let positions = HeaderPositions::new(num_entries as usize);
        let current_position = positions.position_start_entries as u64;
        Ok(Self {
            file: Arc::new(RwLock::new(buf_file)),
            state: DirState::SeekNextPosition,
            current_entry: None,
            count_entry: 0,
            num_entries,
            bucket,
            positions,
            current_position,
            flag: 0,
            entry_name: Vec::new(),
            current_buffer_content: [0u8; 32],
            current_buffer_path: Vec::new(),
        })
    }
}

impl Stream for DirStream {
    type Item = Result<BucketItem, errors::ReadError>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        if this.count_entry >= this.num_entries {
            return Poll::Ready(None);
        }
        match this.state {
            DirState::SeekNextPosition => {
                let waker = cx.waker().clone();
                let file = this.file.clone();
                let pos = this.current_position;
                let future = async move {
                    let result = file.write().await.seek(SeekFrom::Start(pos)).await;
                    waker.wake();
                    result
                };
                let future = Box::pin(future);
                this.state = DirState::WaitingSeekPosition(future);
                cx.waker().wake_by_ref();
                Poll::Pending
            },
            DirState::WaitingSeekPosition(ref mut fut) => {
                let result = fut.as_mut().poll(cx);
                on_future!(this, result, DirState::Eof, |r| {
                    this.state = DirState::ReadFlagEntry;
                    cx.waker().wake_by_ref();
                    Poll::Pending
                })
            },
            DirState::ReadFlagEntry => {
                let waker = cx.waker().clone();
                let file = this.file.clone();
                let future = async move {
                    let result = file.write().await.read_u8().await;
                    waker.wake();
                    result
                };
                let future = Box::pin(future);
                this.state = DirState::WaitingReadFlagEntry(future);
                cx.waker().wake_by_ref();
                Poll::Pending
            },
            DirState::WaitingReadFlagEntry(ref mut fut) => {
                let result = fut.as_mut().poll(cx);
                on_future!(this, result, DirState::Eof, |r| {
                    this.flag = r;
                    this.state = DirState::ReadNameEntry;
                    cx.waker().wake_by_ref();
                    Poll::Pending
                })
            },
            DirState::ReadNameEntry => {
                let waker = cx.waker().clone();
                let file = this.file.clone();
                let future = async move {
                    let mut entry_name = vec![];
                    let result = file.write().await.read_until(0x00, &mut entry_name).await?;
                    waker.wake();
                    Ok(entry_name)
                };
                let future = Box::pin(future);
                this.state = DirState::WaitingReadNameEntry(future);
                cx.waker().wake_by_ref();
                Poll::Pending
            },
            DirState::WaitingReadNameEntry(ref mut fut) => {
                let result = fut.as_mut().poll(cx);
                on_future!(this, result, DirState::Eof, |name| {
                    this.entry_name = name;
                    this.entry_name.pop();
                    if this.flag == B3_DIR_IS_SYM_LINK {
                        this.state = DirState::ReadPathEntry;
                    } else {
                        this.state = DirState::ReadContentEntry;
                    }
                    cx.waker().wake_by_ref();
                    Poll::Pending
                })
            },
            DirState::ReadContentEntry => {
                // Read content hash for regular entries
                let waker = cx.waker().clone();
                let file = this.file.clone();
                let future = async move {
                    let mut buff_content = [0u8; 32];
                    let result = file.write().await.read_exact(&mut buff_content).await?;
                    waker.wake();
                    Ok(buff_content)
                };
                let future = Box::pin(future);
                this.state = DirState::WaitingContentEntry(future);
                cx.waker().wake_by_ref();
                Poll::Pending
            },
            DirState::WaitingContentEntry(ref mut fut) => {
                let result = fut.as_mut().poll(cx);
                on_future!(this, result, DirState::Eof, |content| {
                    this.current_buffer_content = content;
                    this.current_position +=
                        this.entry_name.len() as u64 + this.current_buffer_content.len() as u64 + 2;
                    this.state = DirState::OpenContentEntry;
                    cx.waker().wake_by_ref();
                    Poll::Pending
                })
            },
            DirState::OpenContentEntry => {
                let waker = cx.waker().clone();
                let path = this
                    .bucket
                    .get_header_path(&this.current_buffer_content)
                    .clone();
                let future = async move {
                    let mut file = File::open(path.clone()).await?;
                    let ty = file.read_u8().await?;
                    waker.wake();
                    Ok((path, ty))
                };
                let future = Box::pin(future);
                this.state = DirState::WaitingOpenContentEntry(future);
                cx.waker().wake_by_ref();
                Poll::Pending
            },
            DirState::WaitingOpenContentEntry(ref mut fut) => {
                let result = fut.as_mut().poll(cx);
                on_future!(this, result, DirState::Eof, |(path, ty): (PathBuf, u8)| {
                    this.state = DirState::SeekNextPosition;
                    if ty == HEADER_TYPE_DIR {
                        Poll::Ready(Some(Ok(BucketItem::Dir(DirItem::Dir(path.clone())))))
                    } else {
                        Poll::Ready(Some(Ok(BucketItem::Dir(DirItem::File(path)))))
                    }
                })
            },
            DirState::ReadPathEntry => {
                // Read symlink path until null terminator
                let waker = cx.waker().clone();
                let file = this.file.clone();
                let future = async move {
                    let mut curr_buffer_path = vec![];
                    let result = file
                        .write()
                        .await
                        .read_until(0x00, &mut curr_buffer_path)
                        .await?;
                    waker.wake();
                    Ok(curr_buffer_path) as Result<Vec<u8>, std::io::Error>
                };
                let future = Box::pin(future);
                this.state = DirState::WaitingReadPathEntry(future);
                cx.waker().wake_by_ref();
                Poll::Pending
            },
            DirState::WaitingReadPathEntry(ref mut fut) => {
                let result = fut.as_mut().poll(cx);
                on_future!(this, result, DirState::Eof, |path| {
                    this.current_buffer_path = path;
                    this.state = DirState::SeekNextPosition;
                    this.current_position +=
                        this.entry_name.len() as u64 + this.current_buffer_path.len() as u64 + 2;
                    let dir_item = DirItem::Symblink(this.current_buffer_path.clone());
                    Poll::Ready(Some(Ok(BucketItem::Dir(dir_item))))
                })
            },
            DirState::Eof | DirState::Done => Poll::Ready(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::env::temp_dir;

    use rand::random;
    use tokio_stream::StreamExt;

    use self::bucket::dir::writer::DirWriter;
    use self::bucket::file::writer::FileWriter;
    use super::*;
    use crate::bucket::tests::get_random_file;
    use crate::bucket::Bucket;
    use crate::entry::{BorrowedLink, InlineVec, OwnedLink};
    use crate::utils::to_hex;

    #[tokio::test]
    async fn test_file_stream_new() {
        let random_file_name: [u8; 32] = random();
        let random_file_name = to_hex(&random_file_name);
        let path = temp_dir().join(format!("test_file_stream_new_{}", random_file_name));
        let bucket = Bucket::open(path.clone()).await.unwrap();

        let mut file_writer = FileWriter::new(&bucket).await.unwrap();
        let bytes = get_random_file(8192 * 2);
        file_writer.write(&bytes).await.unwrap();
        let hash = file_writer.commit().await.unwrap();
        let file = File::open(bucket.get_header_path(&hash)).await.unwrap();

        let mut result = FileStream::new(file, bucket).await.unwrap();
        let mut count = 0;
        let mut vec = vec![];
        while let Some(r) = result.next().await {
            count += 1;
            assert!(r.is_ok());
            match r.unwrap() {
                BucketItem::File(f) => vec.extend_from_slice(&f.0),
                BucketItem::Dir(_) => unreachable!("Not possible"),
            }
        }
        assert_eq!(count, 2);
        assert_eq!(vec, bytes);
        tokio::fs::remove_dir(path).await;
    }

    #[tokio::test]
    async fn test_dir_stream_new() {
        let random_file_name: [u8; 32] = random();
        let random_file_name = to_hex(&random_file_name);
        let path = temp_dir().join(format!("test_dir_stream_new_{}", random_file_name));
        let bucket = Bucket::open(path.clone()).await.unwrap();

        let mut vec = vec![];
        for i in 0..5 {
            let mut file_writer = FileWriter::new(&bucket).await.unwrap();
            let bytes = get_random_file(8192 * 2);
            file_writer.write(&bytes).await.unwrap();
            let hash = file_writer.commit().await.unwrap();
            let name: InlineVec = format!("file_{}", i).as_bytes().into();
            let entry = OwnedEntry {
                name,
                link: OwnedLink::Content(hash),
            };
            vec.push(entry);
        }

        let mut dir_writer = DirWriter::new(&bucket, 5).await.unwrap();
        for entry in vec.iter() {
            dir_writer.insert(entry).await.unwrap();
        }

        let hash = dir_writer.commit().await.unwrap();
        let file = File::open(bucket.get_header_path(&hash)).await.unwrap();

        let mut result = DirStream::new(file, bucket).await.unwrap();
        let mut count = 0;
        while let Some(r) = result.next().await {
            count += 1;
            assert!(r.is_ok());
            match r.unwrap() {
                BucketItem::File(_) => unreachable!("Not possible"),
                BucketItem::Dir(_) => (),
            }
        }
        assert_eq!(count, 5);
        tokio::fs::remove_dir(path).await;
    }
}
