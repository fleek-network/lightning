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
}

/// Iterator over entries in a B3 directory
pub struct DirEntriesIter<'a> {
    /// Buffered reader for the directory file
    reader: BufReader<File>,
    /// Current position in the file
    position: u64,
    /// Current state of the iterator state machine
    state: State,
    /// Buffer for entry name being read
    entry_name: Vec<u8>,
    /// Buffer for content hash being read
    current_buffer_content: [u8; 32],
    /// Buffer for symlink path being read
    current_buffer_link: Vec<u8>,
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
            reader,
            position: position_start_entries,
            state: State::ReadPosition,
            entry_name: Vec::new(),
            current_buffer_content: [0u8; 32],
            current_buffer_link: Vec::new(),
            flag: 0,
            _marker: std::marker::PhantomData,
        })
    }
}

/// Macro for handling EOF conditions when reading from the directory file
#[macro_use]
macro_rules! check_eof {
    ($self:expr, $flag:expr) => {
        if let Err(e) = $flag {
            if e.kind() == std::io::ErrorKind::UnexpectedEof {
                $self.state = State::Eof;
                return Poll::Ready(None);
            } else {
                return Poll::Ready(Some(Err(errors::ReadError::from(e))));
            }
        }
    };
}

impl<'a> Stream for DirEntriesIter<'a> {
    type Item = Result<BorrowedEntry<'a>, errors::ReadError>;

    /// Polls the iterator for the next directory entry
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.as_mut().get_mut();

        loop {
            match this.state {
                State::Eof => return Poll::Ready(None),
                State::ReadPosition => {
                    // Reset buffers and seek to next entry position
                    this.entry_name.clear();
                    this.current_buffer_content = [0u8; 32];
                    this.current_buffer_link.clear();
                    this.flag = 0;
                    let future = this.reader.seek(tokio::io::SeekFrom::Start(this.position));
                    let mut future_mut = std::pin::pin!(future);
                    match future_mut.as_mut().poll(cx) {
                        Poll::Ready(result) => {
                            check_eof!(this, result);
                            this.state = State::ReadFlag;
                        },
                        Poll::Ready(Err(e)) => {
                            return Poll::Ready(Some(Err(errors::ReadError::from(e))));
                        },
                        Poll::Pending => return Poll::Pending,
                    }
                },
                State::ReadFlag => {
                    // Read entry type flag
                    let future = this.reader.read_u8();
                    let mut future_mut = std::pin::pin!(future);
                    match future_mut.as_mut().poll(cx) {
                        Poll::Ready(result) => {
                            check_eof!(this, result);
                            match result {
                                Ok(flag_result) => {
                                    this.flag = flag_result;
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
                    // Read entry name until null terminator
                    let future = this.reader.read_until(0x00, &mut this.entry_name);
                    let mut future_mut = std::pin::pin!(future);
                    match future_mut.as_mut().poll(cx) {
                        Poll::Ready(result) => {
                            check_eof!(this, result);
                            match result {
                                Ok(_) => {
                                    this.entry_name.pop();
                                    if this.flag == B3_DIR_IS_SYM_LINK {
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
                    // Read content hash for regular entries
                    let future = this.reader.read_exact(&mut this.current_buffer_content);
                    let mut future_mut = std::pin::pin!(future);
                    match future_mut.as_mut().poll(cx) {
                        Poll::Ready(result) => {
                            check_eof!(this, result);
                            match result {
                                Ok(_) => {
                                    this.state = State::ReadPosition;
                                    this.position += this.entry_name.len() as u64
                                        + this.current_buffer_content.len() as u64
                                        + 2;
                                    let static_slice: &'static [u8; 32] = unsafe {
                                        mem::transmute_copy(&this.current_buffer_content)
                                    };
                                    return Poll::Ready(Some(Ok(BorrowedEntry {
                                        name: Box::leak(this.entry_name.clone().into_boxed_slice()),
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
                    // Read symlink path until null terminator
                    let future = this.reader.read_until(0x00, &mut this.current_buffer_link);
                    let mut future_mut = std::pin::pin!(future);
                    match future_mut.as_mut().poll(cx) {
                        Poll::Ready(result) => {
                            check_eof!(this, result);
                            match result {
                                Ok(_) => {
                                    this.state = State::ReadPosition;
                                    this.position += this.entry_name.len() as u64
                                        + this.current_buffer_link.len() as u64
                                        + 2;
                                    let content = this.current_buffer_link
                                        [..this.current_buffer_link.len() - 1]
                                        .to_vec()
                                        .into_boxed_slice();
                                    let static_slice: &'static [u8] = Box::leak(content);
                                    return Poll::Ready(Some(Ok(BorrowedEntry {
                                        name: Box::leak(this.entry_name.clone().into_boxed_slice()),
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
