use std::{fmt::Debug, ops::Deref};

use async_trait::async_trait;
use thiserror::Error;

use crate::{
    compression::{CompressionAlgoSet, CompressionAlgorithm},
    config::ConfigConsumer,
};

pub type Blake3Hash = [u8; 32];

pub struct Blake3Tree(pub Vec<Blake3Hash>);

/// A chunk of content (usually 256KiB) with a compression tag which determines
/// the compression algorithm that was used to compress this data.
pub struct ContentChunk {
    pub compression: CompressionAlgorithm,
    pub content: Vec<u8>,
}

/// The block store is the local unit on a single node responsible for storing a file, each file in
/// Fleek Network is determined and addressed by its Blake3 hash, we have made this choice to allow
/// us to perform incremental verification over an stream of the content, along with performance
/// benefits.
///
/// To understand the requirements of the BlockStore it is necessary to understand a little bit
/// about Blake3 and how it hashes a file.
///
/// Imagine we give a 5KiB file to Blake3 to hash, the algorithm chunks the file into chunks of
/// 1024 byte.
///
/// ```txt
/// [CHUNK 0] [CHUNK 1] [CHUNK 2] [CHUNK 3] [CHUNK 4]
/// ```
///
/// The next step is to perform some internal hashing algorithm on each chunk, but we also provide
/// the `chunk counter` to the hash function as a domain separator. (So two exact chunks will not
/// have the same hash.)
///
/// ```txt
/// [H(CHUNK 0)] [H(CHUNK 1)] [H(CHUNK 2)] [H(CHUNK 3)] [H(CHUNK 4)]
/// ==
/// [H0] [H1] [H2] [H3] [H4]
/// ```
///
/// The next step of the algorithm uses what we can refer to as a merge operator, the merge
/// function will get 2 hashes and produce another hash. And we use this merge operation
/// on each two chunk to obtain the root hash (`M3`), the technical constraint here is that
/// the left subtree must always be a full-binary tree with exactly `2^n` chunks.
///
/// ```txt
///                              [M3=M(M2, H4)]
///                 [M2=M(M0, M1)]             [H4]
///    [M0=M(H0, H1)]            [M1=M(H2, H3)]
/// [H0]         [H1]         [H2]            [H3]
/// ```
///
/// Now if we store this internal representation we can generate proofs, just like a normal
/// Merkle tree and provide these proofs to the user.
///
/// However, due to real world constraints we cannot deal with resolution of 1024 bytes which
/// leads to large trees, but we instead use chunks of 256KiB, and store the tree only at the
/// depth that would result to each leaf being 256KiB instead.
///
/// We store this tree in a flat vector ([`Blake3Tree`]), which is the stack ordering of the
/// tree:
///
/// ```txt
/// Tree = [H0, H1, M0, H2, H3, M1, M2, H4, M3]
/// ```
///
/// And we have utility functions to traverse this flat structure and provide small incremental
/// proofs.
///
/// Then the act of retrieving a file, is to query the block store for the [`Blake3Tree`]
/// associated with a given `CID` and then we traverse this tree for the chunk leafs (in
/// our example that's anything starting with `H*`), avoiding any internal leaf (anything
/// staring with `M*` in our example), and the to query the block store for the data associated
/// with a chunk.
///
/// Each chunk is it's own independent *content*, so for example if we use a compression algorithm
/// we use it at the chunk level, we don't compress the entire file and then perform the chunking,
/// we chunk first, and compress each chunk later for obvious technical reasons.
#[async_trait]
pub trait BlockStoreInterface: Clone + Send + Sync + ConfigConsumer {
    /// The block store has the ability to use a smart pointer to avoid duplicating
    /// the same content multiple times in memory, this can be used for when multiple
    /// services want access to the same buffer of data.
    type SharedPointer<T: ?Sized + Send + Sync>: Deref<Target = T> + Clone + Send + Sync;

    /// The incremental putter which can be used to write a file to block store.
    type Put: IncrementalPutInterface;

    /// Create a new block store from the given configuration values.
    async fn init(config: Self::Config) -> anyhow::Result<Self>;

    /// Returns the Blake3 tree associated with the given CID. Returns [`None`] if the content
    /// is not present in our block store.
    async fn get_tree(&self, cid: &Blake3Hash) -> Option<Self::SharedPointer<Blake3Tree>>;

    /// Returns the content associated with the given hash and block number, the compression
    /// set determines which compression modes we care about.
    ///
    /// The strongest compression should be preferred. The cache logic should take note of
    /// the number of requests for a `CID` + the supported compression so that it can optimize
    /// storage by storing the compressed version.
    ///
    /// If the content is requested with an empty compression set, the decompressed content is
    /// returned.
    async fn get(
        &self,
        block_counter: u32,
        block_hash: &Blake3Hash,
        compression: CompressionAlgoSet,
    ) -> Option<Self::SharedPointer<ContentChunk>>;

    /// Create a putter that can be used to write a content into the block store.
    fn put(&self, cid: Option<Blake3Hash>) -> Self::Put;
}

/// The interface for the writer to a [`BlockStoreInterface`].
#[async_trait]
pub trait IncrementalPutInterface {
    /// Write the proof for the buffer.
    fn feed_proof(&mut self, proof: &[u8]) -> Result<(), PutFeedProofError>;

    /// Write the content. If there has been a call to `write_proof`, an incremental
    /// validation will happen.
    fn write(
        &mut self,
        content: &[u8],
        compression: CompressionAlgorithm,
    ) -> Result<(), PutWriteError>;

    /// Returns true if the writer is not expecting any more bytes.
    fn is_finished(&self) -> bool;

    /// Finalize the write, try to write all of the content to the file system or any other
    /// underlying storage medium used to implement the [`BlockStoreInterface`].
    async fn finalize(self) -> Result<Blake3Hash, PutFinalizeError>;
}

#[derive(Error, Debug)]
pub enum PutFeedProofError {
    #[error("Putter was running without incremental verification.")]
    UnexpectedCall,
    #[error("Proof is not matching the current root.")]
    InvalidProof,
}

#[derive(Error, Debug)]
pub enum PutWriteError {
    #[error("The provided content is not matching the hash.")]
    InvalidContent,
    #[error("The provided content could not be decompressed.")]
    DecompressionFailure,
}

#[derive(Error, Debug)]
pub enum PutFinalizeError {
    #[error("The putter expected more data to come to a finalized state.")]
    PartialContent,
    #[error("The final CID does not match the CID which was expected.")]
    InvalidCID,
}
