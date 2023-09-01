use std::io;
use std::marker::PhantomData;
use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use blake3_tree::blake3::Hash;
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::types::{CompressionAlgoSet, CompressionAlgorithm};
use lightning_interfaces::{
    Blake3Hash,
    Blake3Tree,
    BlockStoreInterface,
    ConfigConsumer,
    ContentChunk,
};
use resolved_pathbuf::ResolvedPathBuf;
use serde::{Deserialize, Serialize};
use tempdir::TempDir;
use tokio::fs::{self, File};
use tokio::io::AsyncWriteExt;

use crate::put::Putter;
use crate::store::Store;
use crate::{Block, BlockContent, Key};
