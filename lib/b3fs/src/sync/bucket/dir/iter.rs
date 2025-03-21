use std::fs::File;
use std::future::Future;
use std::io::{BufRead, BufReader, ErrorKind, Read, Seek};
use std::iter::Iterator;
use std::mem;
use std::pin::Pin;
use std::sync::{Arc, RwLock};
use std::task::{Context, Poll};

use crate::entry::{BorrowedEntry, BorrowedLink, OwnedEntry, OwnedLink};
use crate::hasher::dir_hasher::B3_DIR_IS_SYM_LINK;
use crate::on_future;
use crate::sync::bucket::errors;

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
pub struct DirEntriesIter {
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
}

unsafe impl Send for DirEntriesIter {}
unsafe impl Sync for DirEntriesIter {}

impl DirEntriesIter {
    /// Creates a new directory entries iterator starting at the given position
    pub fn new(reader: File, position_start_entries: u64) -> Result<Self, errors::ReadError> {
        let mut reader = BufReader::new(reader);
        Ok(Self {
            reader: Arc::new(RwLock::new(reader)),
            position: position_start_entries,
            state: State::ReadPosition,
            entry_name: Vec::new(),
            flag: 0,
        })
    }
}

impl Iterator for DirEntriesIter {
    type Item = Result<OwnedEntry, errors::ReadError>;

    fn next(&mut self) -> Option<Self::Item> {
        let file = self.reader.clone();

        let Ok(mut file) = file.write() else {
            return Some(Err(errors::ReadError::LockError));
        };
        if let Err(e) = file.seek(std::io::SeekFrom::Start(self.position)) {
            if e.kind() == std::io::ErrorKind::UnexpectedEof {
                return None;
            } else {
                return Some(Err(errors::ReadError::from(e)));
            }
        };

        let mut buf = [0; 1];
        let result = file.read_exact(&mut buf);

        if let Err(e) = file.read_exact(&mut buf) {
            if e.kind() == std::io::ErrorKind::UnexpectedEof {
                return None;
            } else {
                return Some(Err(errors::ReadError::from(e)));
            }
        };
        let flag = buf[0];

        let mut name_buffer = Vec::new();
        let size = match file.read_until(0x00, &mut name_buffer) {
            Ok(size) => size,
            Err(e) => {
                if e.kind() == std::io::ErrorKind::UnexpectedEof {
                    return None;
                } else {
                    return Some(Err(errors::ReadError::from(e)));
                }
            },
        };

        // Remove the marker bit 0x00
        name_buffer.pop();
        let entry_name = name_buffer;

        if flag & B3_DIR_IS_SYM_LINK != 0 {
            //State::ReadPath
            let mut path_buffer = Vec::new();
            let size = match file.read_until(0x00, &mut path_buffer) {
                Ok(size) => size,
                Err(e) => {
                    if e.kind() == std::io::ErrorKind::UnexpectedEof {
                        return None;
                    } else {
                        return Some(Err(errors::ReadError::from(e)));
                    }
                },
            };

            self.position += entry_name.len() as u64 + size as u64 + 2;
            // Remove the marker bit 0x00
            let content = path_buffer[0..size - 1].to_vec();

            Some(Ok(OwnedEntry {
                name: entry_name.into(),
                link: OwnedLink::Link(smallvec::SmallVec::from_vec(content)),
            }))
        } else {
            //State::ReadContent
            let mut content_buffer = [0u8; 32];
            if let Err(e) = file.read_exact(&mut content_buffer) {
                if e.kind() == std::io::ErrorKind::UnexpectedEof {
                    return None;
                } else {
                    return Some(Err(errors::ReadError::from(e)));
                }
            };
            self.position += entry_name.len() as u64 + content_buffer.len() as u64 + 2;

            Some(Ok(OwnedEntry {
                name: entry_name.clone().into(),
                link: OwnedLink::Content(content_buffer),
            }))
        }
    }
}
