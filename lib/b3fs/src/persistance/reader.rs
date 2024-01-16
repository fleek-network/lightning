use crate::collections::HashTree;
use crate::persistance::dir_reader::DirectoryReader;

/// The deserializer of an on-disk header file which can be either a directory or a file.
pub struct Reader<'b> {
    /// At this layer the first 4 bytes will tell us what serialization to use. Although
    /// even a single byte would be enough the reason to chose the number 4 here is so
    /// that the actual content to be serialized would have an alignment of 4.
    buffer: &'b [u8],
}

impl<'b> Reader<'b> {
    /// Returns a new [`Header`] from the provided buffer. This performs no validation creating
    /// a header from invalid source can cause errors and possible panics in futures calls to
    /// the methods downstream.
    #[inline]
    pub fn new(buffer: &'b [u8]) -> Self {
        Self { buffer }
    }

    /// Returns true if this header belongs to a file and not a directory.
    #[inline]
    pub fn is_file(&self) -> bool {
        self.buffer[0] == 0x00
    }

    /// Returns true if this header belongs to a directory.
    #[inline]
    pub fn is_directory(&self) -> bool {
        self.buffer[0] == 0x01
    }

    /// Returns `Some` containing the [`HashTree`] of a file if the current header indicates
    /// a file.
    #[inline]
    pub fn as_file(&self) -> Option<HashTree<'b>> {
        self.is_file()
            .then(|| HashTree::try_from(&self.buffer[4..]).expect("Header to be of valid size."))
    }

    /// Returns `Some` containing the deserializer of the directory.
    #[inline]
    pub fn as_directory(&self) -> Option<DirectoryReader> {
        self.is_directory()
            .then(|| DirectoryReader::new(&self.buffer[4..]))
    }
}
