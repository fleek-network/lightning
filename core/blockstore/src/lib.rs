mod config;
mod memory;
mod put;

use std::sync::Arc;

use draco_interfaces::{Blake3Hash, Blake3Tree, ContentChunk};

#[derive(Hash, Eq, PartialEq)]
pub struct Key(Blake3Hash, Option<u32>);

pub enum Block {
    Tree(Arc<Blake3Tree>),
    Chunk(Arc<ContentChunk>),
}

#[cfg(test)]
mod tests {
    use blake3_tree::blake3::tree::HashTreeBuilder;
    use draco_interfaces::{
        Blake3Hash, BlockStoreInterface, CompressionAlgorithm, IncrementalPutInterface,
    };
    use tokio::test;

    use crate::{config::Config, memory::MemoryBlockStore};

    #[test]
    async fn test_put() {
        let content = (0..4)
            .map(|i| Vec::from([i; 256 * 1024]))
            .flat_map(|a| a.into_iter())
            .collect::<Vec<_>>();

        let blockstore = MemoryBlockStore::init(Config).await.unwrap();
        let mut putter = blockstore.put(None);
        putter
            .write(content.as_slice(), CompressionAlgorithm::Uncompressed)
            .unwrap();
        let root = putter.finalize().await.unwrap();

        let mut tree_builder = HashTreeBuilder::new();
        (0..4).for_each(|i| tree_builder.update(&[i; 256 * 1024]));
        let output = tree_builder.finalize();

        assert_eq!(root, Blake3Hash::from(output.hash));
    }

    #[test]
    async fn test_put_verify() {
        let content = (0..4)
            .map(|i| Vec::from([i; 256 * 1024]))
            .flat_map(|a| a.into_iter())
            .collect::<Vec<_>>();

        let blockstore = MemoryBlockStore::init(Config).await.unwrap();
        let mut putter = blockstore.put(None);
        putter
            .write(content.as_slice(), CompressionAlgorithm::Uncompressed)
            .unwrap();

        let root = putter.finalize().await.unwrap();
        let mut tree_builder = HashTreeBuilder::new();
        (0..4).for_each(|i| tree_builder.update(&[i; 256 * 1024]));
        let output = tree_builder.finalize();
        assert_eq!(root, Blake3Hash::from(output.hash));

        let content = (0..4)
            .map(|i| Vec::from([i; 256 * 1024]))
            .flat_map(|a| a.into_iter())
            .collect::<Vec<_>>();

        let mut putter = blockstore.put(None);
        putter.feed_proof(output.hash.as_bytes()).unwrap();
        putter
            .write(content.as_slice(), CompressionAlgorithm::Uncompressed)
            .unwrap();
        let root = putter.finalize().await.unwrap();
        assert_eq!(root, Blake3Hash::from(output.hash));
    }
}
