extern crate core;

mod config;
mod memory;
mod put;

use std::sync::Arc;

use draco_interfaces::{Blake3Hash, Blake3Tree, ContentChunk};

#[derive(Hash, Eq, PartialEq, Debug)]
pub struct Key(Blake3Hash, Option<u32>);

pub enum Block {
    Tree(Arc<Blake3Tree>),
    Chunk(Arc<ContentChunk>),
}

#[cfg(test)]
mod tests {
    use blake3_tree::blake3::tree::{BlockHasher, HashTree, HashTreeBuilder};
    use draco_interfaces::{
        Blake3Hash, BlockStoreInterface, CompressionAlgoSet, CompressionAlgorithm,
        IncrementalPutInterface,
    };
    use tokio::test;

    use crate::{config::Config, memory::MemoryBlockStore};

    fn create_content() -> Vec<u8> {
        (0..4)
            .map(|i| Vec::from([i; 256 * 1024]))
            .flat_map(|a| a.into_iter())
            .collect()
    }

    fn hash_tree(content: &[u8]) -> HashTree {
        let mut tree_builder = HashTreeBuilder::new();
        tree_builder.update(content);
        let tree_hash = tree_builder.finalize();
        tree_hash
    }

    #[test]
    async fn test_put() {
        // Given: some content.
        let content = create_content();
        // Given: a block store.
        let blockstore = MemoryBlockStore::init(Config).await.unwrap();
        // When: we create a putter and write some content.
        let mut putter = blockstore.put(None);
        putter
            .write(content.as_slice(), CompressionAlgorithm::Uncompressed)
            .unwrap();
        // Then: the putter returns the appropriate root hash.
        let root = putter.finalize().await.unwrap();
        let expected_root = hash_tree(content.as_slice()).hash;
        assert_eq!(root, Blake3Hash::from(expected_root));
    }

    #[test]
    async fn test_put_verify() {
        // Given: some content.
        let content = create_content();
        // Given: a block store.
        let blockstore = MemoryBlockStore::init(Config).await.unwrap();
        // Given: we put the content in the block store.
        let mut putter = blockstore.put(None);
        putter
            .write(content.as_slice(), CompressionAlgorithm::Uncompressed)
            .unwrap();
        putter.finalize().await.unwrap();
        // When: we put the same content and feed the proof to verify it.
        let mut putter = blockstore.put(None);
        let expected_root = hash_tree(content.as_slice()).hash;
        putter.feed_proof(expected_root.as_bytes()).unwrap();
        putter
            .write(content.as_slice(), CompressionAlgorithm::Uncompressed)
            .unwrap();
        // Then: the putter returns the appropriate root hash and no errors.
        let root = putter.finalize().await.unwrap();
        assert_eq!(root, Blake3Hash::from(expected_root));
    }

    #[test]
    async fn test_put_chunks() {
        // Given: some content.
        let content = create_content();
        // Given: a block store.
        let blockstore = MemoryBlockStore::init(Config).await.unwrap();
        // When: we create a putter and write some content in chunks smaller than blake3 chunks.
        let mut putter = blockstore.put(None);
        for chunk in content.chunks(128) {
            putter
                .write(chunk, CompressionAlgorithm::Uncompressed)
                .unwrap();
        }
        // Then: the putter returns the appropriate root hash.
        let root = putter.finalize().await.unwrap();
        let expected_root = hash_tree(content.as_slice()).hash;
        assert_eq!(root, Blake3Hash::from(expected_root));
    }

    #[test]
    async fn test_put_chunks_verify() {
        // Given: some content.
        let content = create_content();
        // Given: a block store.
        let blockstore = MemoryBlockStore::init(Config).await.unwrap();
        // Given: we put the content in the block store.
        let mut putter = blockstore.put(None);
        putter
            .write(content.as_slice(), CompressionAlgorithm::Uncompressed)
            .unwrap();
        putter.finalize().await.unwrap();
        // When: feed the proof to verify our content and pass the content in chunks.
        let mut putter = blockstore.put(None);
        let expected_root = hash_tree(content.as_slice()).hash;
        putter.feed_proof(expected_root.as_bytes()).unwrap();
        for chunk in content.chunks(128) {
            putter
                .write(chunk, CompressionAlgorithm::Uncompressed)
                .unwrap();
        }
        // Then: the putter returns the appropriate root hash and no errors.
        let root = putter.finalize().await.unwrap();
        assert_eq!(root, Blake3Hash::from(expected_root));
    }

    #[test]
    async fn test_get() {
        // Given: some content.
        let content = create_content();
        // Given: a block store.
        let blockstore = MemoryBlockStore::init(Config).await.unwrap();
        // Given: we put the content in the block store.
        let mut putter = blockstore.put(None);
        putter
            .write(content.as_slice(), CompressionAlgorithm::Uncompressed)
            .unwrap();
        putter.finalize().await.unwrap();
        // When: we query the block store for our blocks using their hashes.
        for (count, chunk) in content.chunks(256 * 1024).enumerate() {
            let mut block = BlockHasher::new();
            block.set_block(count);
            block.update(chunk);
            let hash = block.finalize(true);
            let content_from_store = blockstore
                .get(count as u32, &hash, CompressionAlgoSet::new())
                .await
                .unwrap();
            // Then: we get our content as expected.
            assert_eq!(content_from_store.content, chunk);
        }
    }
}
