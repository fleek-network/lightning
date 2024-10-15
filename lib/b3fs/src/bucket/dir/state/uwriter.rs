use std::num::NonZeroU32;
use std::os::unix::fs::FileExt;
use std::path::{Path, PathBuf};

use bytes::BytesMut;
use fastbloom_rs::{FilterBuilder, Membership};
use rand::random;
use serde::Serialize as _;
use tokio::fs::{File, OpenOptions};
use tokio::io::{self, AsyncWriteExt, BufWriter};

use super::*;
use crate::bucket::dir::phf::{HasherState, PhfGenerator};
use crate::bucket::{errors, Bucket};
use crate::entry::{BorrowedEntry, BorrowedLink};
use crate::hasher::b3::MAX_BLOCK_SIZE_IN_BYTES;
use crate::hasher::collector::BufCollector;
use crate::hasher::dir_hasher::DirectoryHasher;
use crate::hasher::HashTreeCollector;
use crate::stream::verifier::{IncrementalVerifier, WithHashTreeCollector};
use crate::utils;

#[derive(Default)]
pub(crate) struct DirUWriterCollector {
    hasher: IncrementalVerifier<WithHashTreeCollector>,
    root_hash: [u8; 32],
}
impl DirUWriterCollector {
    pub(crate) fn new(hash: [u8; 32]) -> Self {
        Self {
            hasher: IncrementalVerifier::<WithHashTreeCollector>::dir(),
            root_hash: hash,
        }
    }

    pub(crate) fn feed_proof(&mut self, proof: &[u8]) -> Result<(), errors::FeedProofError> {
        self.hasher.feed_proof(proof).map_err(Into::into)
    }
}

impl WithCollector for DirUWriterCollector {
    async fn on_insert(
        &mut self,
        borrowed_entry: BorrowedEntry<'_>,
    ) -> Result<(), errors::InsertError> {
        let mut dir_hasher: DirectoryHasher<Vec<[u8; 32]>> = DirectoryHasher::default();
        dir_hasher.insert_unchecked(borrowed_entry);
        let (_, tree) = dir_hasher.finalize();
        self.hasher
            .verify_hash(tree[0])
            .map_err(|_| errors::InsertError::IncrementalVerification)
    }

    async fn on_commit(self) -> Result<([u8; 32], Vec<[u8; 32]>), errors::CommitError> {
        Ok((self.root_hash, self.hasher.finalize()))
    }
}

pub type DirUWriterState = InnerDirState<DirUWriterCollector>;
