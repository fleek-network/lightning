use std::future::Future;
use std::mem;
use std::pin::Pin;
use std::task::{Context, Poll};

use tokio::fs::File;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncSeekExt, BufReader};
use tokio_stream::Stream;

use crate::bucket::errors;
use crate::entry::{BorrowedEntry, BorrowedLink};
use crate::hasher::dir_hasher::B3_DIR_IS_SYM_LINK;

enum State {
    ReadFlag,
    ReadEntryName,
    ReadContent,
    ReadPath,
}

pub struct DirEntriesIter<'a> {
    reader: BufReader<File>,
    state: State,
    _marker: std::marker::PhantomData<&'a ()>,
}

impl<'a> DirEntriesIter<'a> {
    pub async fn new(reader: File, position_start_entries: u64) -> Result<Self, errors::ReadError> {
        let mut reader = BufReader::new(reader);
        reader
            .seek(tokio::io::SeekFrom::Start(position_start_entries))
            .await
            .map_err(|_| errors::ReadError::RefFile)?;
        Ok(Self {
            reader,
            state: State::ReadFlag,
            _marker: std::marker::PhantomData,
        })
    }
}

#[macro_use]
macro_rules! check_eof {
    ($flag:expr) => {
        if let Err(e) = $flag {
            if e.kind() == std::io::ErrorKind::UnexpectedEof {
                return Poll::Ready(None);
            } else {
                return Poll::Ready(Some(Err(errors::ReadError::from(e))));
            }
        }
    };
}

impl<'a> Stream for DirEntriesIter<'a> {
    type Item = Result<BorrowedEntry<'a>, errors::ReadError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.as_mut().get_mut();

        let mut flag = 0;
        let mut entry_name = Vec::new();
        loop {
            match this.state {
                State::ReadFlag => {
                    let future = this.reader.read_u8();
                    let mut future_mut = std::pin::pin!(future);
                    match future_mut.as_mut().poll(cx) {
                        Poll::Ready(result) => {
                            check_eof!(result);
                            match result {
                                Ok(flag_result) => {
                                    flag = flag_result;
                                    this.state = State::ReadEntryName;
                                },
                                Err(e) => {
                                    return Poll::Ready(Some(Err(errors::ReadError::from(e))));
                                },
                            }
                        },
                        Poll::Pending => return Poll::Pending,
                    }
                },
                State::ReadEntryName => {
                    let future = this.reader.read_until(0x00, &mut entry_name);
                    let mut future_mut = std::pin::pin!(future);
                    match future_mut.as_mut().poll(cx) {
                        Poll::Ready(result) => {
                            check_eof!(result);
                            match result {
                                Ok(_) => {
                                    if flag == B3_DIR_IS_SYM_LINK {
                                        this.state = State::ReadPath;
                                    } else {
                                        this.state = State::ReadContent;
                                    }
                                },
                                Err(e) => {
                                    return Poll::Ready(Some(Err(errors::ReadError::from(e))));
                                },
                            }
                        },
                        Poll::Pending => return Poll::Pending,
                    }
                },
                State::ReadContent => {
                    let mut buffer: [u8; 32] = vec![0u8; 32].try_into().unwrap();
                    let future = this.reader.read_exact(&mut buffer);
                    let mut future_mut = std::pin::pin!(future);
                    match future_mut.as_mut().poll(cx) {
                        Poll::Ready(result) => {
                            check_eof!(result);
                            match result {
                                Ok(_) => {
                                    let static_slice: &'static [u8; 32] =
                                        unsafe { mem::transmute_copy(&buffer) };
                                    return Poll::Ready(Some(Ok(BorrowedEntry {
                                        name: Box::leak(entry_name.into_boxed_slice()),
                                        link: BorrowedLink::Content(static_slice),
                                    })));
                                },
                                Err(e) => {
                                    return Poll::Ready(Some(Err(errors::ReadError::from(e))));
                                },
                            }
                        },
                        Poll::Pending => return Poll::Pending,
                    }
                },
                State::ReadPath => {
                    let mut content = Vec::new();
                    let future = this.reader.read_until(0x00, &mut content);
                    let mut future_mut = std::pin::pin!(future);
                    match future_mut.as_mut().poll(cx) {
                        Poll::Ready(result) => {
                            check_eof!(result);
                            match result {
                                Ok(_) => {
                                    let content = content.into_boxed_slice();
                                    let static_slice: &'static [u8] = Box::leak(content);
                                    return Poll::Ready(Some(Ok(BorrowedEntry {
                                        name: Box::leak(entry_name.into_boxed_slice()),
                                        link: BorrowedLink::Path(static_slice),
                                    })));
                                },
                                Err(e) => {
                                    return Poll::Ready(Some(Err(errors::ReadError::from(e))));
                                },
                            }
                        },
                        Poll::Pending => return Poll::Pending,
                    }
                },
            }
        }
        Poll::Ready(None)
    }
}
