mod config;
mod memory;
mod put;

use std::sync::Arc;
use draco_interfaces::{
    Blake3Hash, Blake3Tree,
    ContentChunk
};

#[derive(Hash, Eq, PartialEq)]
pub struct Key<'a>(&'a Blake3Hash, Option<u32>);

pub enum Block {
    Tree(Arc<Blake3Tree>),
    Chunk(Arc<ContentChunk>),
}
