pub mod blockstore;
pub mod config;
pub mod put;
mod store;

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::path::PathBuf;
    use std::time::{Duration, SystemTime};

    use blake3_tree::blake3::tree::{HashTree, HashTreeBuilder};
    use blake3_tree::ProofBuf;
    use fleek_crypto::{AccountOwnerSecretKey, ConsensusSecretKey, NodeSecretKey, SecretKey};
    use lightning_application::app::Application;
    use lightning_application::config::{Config as AppConfig, Mode, StorageConfig};
    use lightning_application::genesis::{Genesis, GenesisNode};
    use lightning_indexer::Indexer;
    use lightning_interfaces::infu_collection::Collection;
    use lightning_interfaces::types::{Blake3Hash, CompressionAlgorithm, NodePorts};
    use lightning_interfaces::{
        partial,
        ApplicationInterface,
        BlockStoreInterface,
        ConsensusInterface,
        IncrementalPutInterface,
        IndexerInterface,
        SignerInterface,
        WithStartAndShutdown,
    };
    use lightning_signer::{Config as SignerConfig, Signer};
    use lightning_test_utils::consensus::{Config as ConsensusConfig, MockConsensus};
    use tokio::test;

    use crate::blockstore::{Blockstore, BLOCK_SIZE};
    use crate::config::Config;

    partial!(TestBinding {
        ApplicationInterface = Application<Self>;
        BlockStoreInterface = Blockstore<Self>;
        SignerInterface = Signer<Self>;
        ConsensusInterface = MockConsensus<Self>;
        IndexerInterface = Indexer<Self>;
    });

    fn create_content() -> Vec<u8> {
        (0..4)
            .map(|i| Vec::from([i; BLOCK_SIZE]))
            .flat_map(|a| a.into_iter())
            .collect()
    }

    fn hash_tree(content: &[u8]) -> HashTree {
        let mut tree_builder = HashTreeBuilder::new();
        tree_builder.update(content);
        tree_builder.finalize()
    }

    fn new_proof(tree: &[[u8; 32]], block: usize) -> ProofBuf {
        if block > 0 {
            ProofBuf::resume(tree, block)
        } else {
            ProofBuf::new(tree, block)
        }
    }

    async fn create_blockstore(
        test_name: String,
    ) -> (Application<TestBinding>, Blockstore<TestBinding>, PathBuf) {
        let signer_config = SignerConfig::test();
        let (consensus_secret_key, node_secret_key) = signer_config.load_test_keys();
        let node_public_key = node_secret_key.to_pk();
        let consensus_public_key = consensus_secret_key.to_pk();
        let owner_secret_key = AccountOwnerSecretKey::generate();
        let owner_public_key = owner_secret_key.to_pk();

        let peer_owner_public_key = AccountOwnerSecretKey::generate();
        let peer_secret_key = NodeSecretKey::generate();
        let peer_public_key = peer_secret_key.to_pk();
        let peer_consensus_secret_key = ConsensusSecretKey::generate();
        let peer_consensus_public_key = peer_consensus_secret_key.to_pk();

        let mut genesis = Genesis::load().unwrap();

        genesis.node_info.push(GenesisNode::new(
            owner_public_key.into(),
            node_public_key,
            "127.0.0.1".parse().unwrap(),
            consensus_public_key,
            "127.0.0.1".parse().unwrap(),
            node_public_key,
            NodePorts {
                primary: 48000,
                worker: 48101,
                mempool: 48102,
                rpc: 48103,
                pool: 48104,
                pinger: 48106,
                handshake: Default::default(),
            },
            None,
            true,
        ));

        genesis.node_info.push(GenesisNode::new(
            peer_owner_public_key.to_pk().into(),
            peer_public_key,
            "127.0.0.1".parse().unwrap(),
            peer_consensus_public_key,
            "127.0.0.1".parse().unwrap(),
            peer_public_key,
            NodePorts {
                primary: 38000,
                worker: 38101,
                mempool: 38102,
                rpc: 38103,
                pool: 38104,
                pinger: 38106,
                handshake: Default::default(),
            },
            None,
            true,
        ));

        let epoch_start = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        genesis.epoch_start = epoch_start;
        genesis.epoch_time = 4000; // millis

        let path = std::env::temp_dir().join(test_name);
        let mut blockstore = Blockstore::<TestBinding>::init(Config {
            root: path.clone().try_into().unwrap(),
        })
        .unwrap();

        let app = Application::<TestBinding>::init(
            AppConfig {
                genesis: Some(genesis),
                mode: Mode::Test,
                testnet: false,
                storage: StorageConfig::InMemory,
                db_path: None,
                db_options: None,
            },
            blockstore.clone(),
        )
        .unwrap();
        app.start().await;

        let (update_socket, query_runner) = (app.transaction_executor(), app.sync_query());

        let mut signer = Signer::<TestBinding>::init(signer_config, query_runner.clone()).unwrap();

        let consensus_config = ConsensusConfig {
            min_ordering_time: 0,
            max_ordering_time: 1,
            probability_txn_lost: 0.0,
            transactions_to_lose: HashSet::new(),
            new_block_interval: Duration::from_secs(5),
        };
        let consensus = MockConsensus::<TestBinding>::init(
            consensus_config,
            &signer,
            update_socket,
            query_runner.clone(),
            infusion::Blank::default(),
            None,
        )
        .unwrap();

        signer.provide_mempool(consensus.mempool());
        signer.provide_new_block_notify(consensus.new_block_notifier());
        signer.start().await;
        consensus.start().await;

        let indexer =
            Indexer::<TestBinding>::init(Default::default(), signer.get_socket()).unwrap();

        blockstore.provide_indexer(indexer);

        (app, blockstore, path)
    }

    #[test]
    async fn test_put_verify() {
        // Given: some content.
        let content = create_content();
        // Given: a block store.
        let (_app, blockstore, path) =
            create_blockstore(format!("test-{}", std::thread::current().name().unwrap())).await;

        let test = async move {
            // Given: we put the content in the block store.
            let mut putter = blockstore.put(None);
            putter
                .write(content.as_slice(), CompressionAlgorithm::Uncompressed)
                .unwrap();
            putter.finalize().await.unwrap();

            // Given: the full tree.
            let hash_tree = hash_tree(content.as_slice());

            // When: we put the content by block and feed the proof to verify it.
            let mut putter = blockstore.put(Some(Blake3Hash::from(hash_tree.hash)));
            for (i, block) in content.chunks(BLOCK_SIZE).enumerate() {
                let proof = new_proof(&hash_tree.tree, i);
                putter.feed_proof(proof.as_slice()).unwrap();
                putter
                    .write(block, CompressionAlgorithm::Uncompressed)
                    .unwrap();
            }

            // Then: the putter returns the appropriate root hash and no errors.
            let root = putter
                .finalize()
                .await
                .map_err(|e| anyhow::anyhow!("{e:?}"))?;
            if root != Blake3Hash::from(hash_tree.hash) {
                anyhow::bail!("invalid root hash");
            }

            Ok(())
        };

        let result = test.await;

        if path.exists() {
            std::fs::remove_dir_all(path).unwrap();
        }

        result.expect("Test to pass");
    }

    #[test]
    async fn test_put_verify_invalid_content() {
        // Given: some content.
        let mut content = create_content();

        // Given: a block store.
        let (_app, blockstore, path) =
            create_blockstore(format!("test-{}", std::thread::current().name().unwrap())).await;

        let test = async move {
            // Given: we put the content in the block store and feed the proof to verify it.
            let mut putter = blockstore.put(None);
            putter
                .write(content.as_slice(), CompressionAlgorithm::Uncompressed)
                .map_err(|e| anyhow::anyhow!("{e:?}"))?;
            putter
                .finalize()
                .await
                .map_err(|e| anyhow::anyhow!("{e:?}"))?;

            // Given: the full tree.
            let hash_tree = hash_tree(content.as_slice());

            // Given: make a change to the content.
            content[10] = 69;

            // When: we put a block with modified content and feed the proof to verify it.
            let mut putter = blockstore.put(Some(Blake3Hash::from(hash_tree.hash)));
            let mut blocks = content.chunks(BLOCK_SIZE);
            let proof = ProofBuf::new(&hash_tree.tree, 0);
            putter
                .feed_proof(proof.as_slice())
                .map_err(|e| anyhow::anyhow!("{e:?}"))?;
            putter
                .write(blocks.next().unwrap(), CompressionAlgorithm::Uncompressed)
                .map_err(|e| anyhow::anyhow!("{e:?}"))?;

            Result::<(), anyhow::Error>::Ok(())
        };

        let result = test.await;

        if path.exists() {
            std::fs::remove_dir_all(path).unwrap();
        }

        assert!(result.is_err());
    }

    #[test]
    async fn test_put_verify_one_chunk() {
        // Given: some content.
        let content = [0; BLOCK_SIZE];

        // Given: a block store.
        let (_app, blockstore, path) =
            create_blockstore(format!("test-{}", std::thread::current().name().unwrap())).await;

        // Given: we put the content in the block store.
        let test = async move {
            let mut putter = blockstore.put(None);
            putter
                .write(&content, CompressionAlgorithm::Uncompressed)
                .map_err(|e| anyhow::anyhow!("{e:?}"))?;
            putter
                .finalize()
                .await
                .map_err(|e| anyhow::anyhow!("{e:?}"))?;

            // Given: the full tree.
            let hash_tree = hash_tree(&content);

            // When: we put one block and feed the proof to verify it.
            let mut putter = blockstore.put(Some(Blake3Hash::from(hash_tree.hash)));
            let proof = new_proof(&hash_tree.tree, 0);
            putter
                .feed_proof(proof.as_slice())
                .map_err(|e| anyhow::anyhow!("{e:?}"))?;
            putter
                .write(&content, CompressionAlgorithm::Uncompressed)
                .map_err(|e| anyhow::anyhow!("{e:?}"))?;

            // Then: the putter returns the appropriate root hash and no errors.
            let root = putter
                .finalize()
                .await
                .map_err(|e| anyhow::anyhow!("{e:?}"))?;
            if root != Blake3Hash::from(hash_tree.hash) {
                anyhow::bail!("Invalid root")
            }

            Ok(())
        };

        let result = test.await;

        if path.exists() {
            std::fs::remove_dir_all(path).unwrap();
        }

        result.expect("Test to pass");
    }

    #[test]
    async fn test_put_verify_one_chunk_small() {
        // Given: some content.
        let content = [0; 256];
        // Given: a block store.
        let (_app, blockstore, path) =
            create_blockstore(format!("test-{}", std::thread::current().name().unwrap())).await;

        let test = async move {
            // Given: we put the content in the block store.
            let mut putter = blockstore.put(None);
            putter
                .write(&content, CompressionAlgorithm::Uncompressed)
                .map_err(|e| anyhow::anyhow!("{e:?}"))?;
            putter
                .finalize()
                .await
                .map_err(|e| anyhow::anyhow!("{e:?}"))?;

            // Given: the full tree.
            let hash_tree = hash_tree(&content);

            // ----

            // When: we put one block and feed the proof to verify it.
            let mut putter = blockstore.put(Some(Blake3Hash::from(hash_tree.hash)));
            let proof = new_proof(&hash_tree.tree, 0);
            putter
                .feed_proof(proof.as_slice())
                .map_err(|e| anyhow::anyhow!("{e:?}"))?;
            putter
                .write(&content, CompressionAlgorithm::Uncompressed)
                .map_err(|e| anyhow::anyhow!("{e:?}"))?;

            // Then: the putter returns the appropriate root hash and no errors.
            let root = putter.finalize().await.unwrap();
            if root != Blake3Hash::from(hash_tree.hash) {
                anyhow::bail!("Invalid root")
            }

            Ok(())
        };

        let result = test.await;

        if path.exists() {
            std::fs::remove_dir_all(path).unwrap();
        }

        result.expect("Test to pass");
    }

    #[test]
    async fn test_put_verify_one_chunk_and_a_half() {
        // Given: one chunk and another chunk smaller than a Blake3 chunk.
        let content = vec![vec![0; 256 * 1024], vec![1; 256]]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();

        // Given: a block store.
        let (_app, blockstore, path) =
            create_blockstore(format!("test-{}", std::thread::current().name().unwrap())).await;

        let test = async move {
            // Given: we put the content in the block store.
            let mut putter = blockstore.put(None);
            putter
                .write(&content, CompressionAlgorithm::Uncompressed)
                .map_err(|e| anyhow::anyhow!("{e:?}"))?;
            putter
                .finalize()
                .await
                .map_err(|e| anyhow::anyhow!("{e:?}"))?;

            // Given: the full tree.
            let hash_tree = hash_tree(&content);

            // When: we put the content by block and feed the proof to verify it.
            let size = blockstore
                .read_all_to_vec(hash_tree.hash.as_bytes())
                .await
                .unwrap()
                .len();
            assert_eq!(size, 262400);

            // When: we verify the content.
            let mut putter = blockstore.put(Some(Blake3Hash::from(hash_tree.hash)));
            let proof = new_proof(&hash_tree.tree, 0);
            putter
                .feed_proof(proof.as_slice())
                .map_err(|e| anyhow::anyhow!("{e:?}"))?;
            putter
                .write(&content, CompressionAlgorithm::Uncompressed)
                .unwrap();

            // Then: the putter returns the appropriate root hash and no errors.
            let root = putter
                .finalize()
                .await
                .map_err(|e| anyhow::anyhow!("{e:?}"))?;

            if root != Blake3Hash::from(hash_tree.hash) {
                anyhow::bail!("Invalid root")
            }

            Ok(())
        };

        let result = test.await;

        if path.exists() {
            std::fs::remove_dir_all(path).unwrap();
        }

        result.expect("Test to pass");
    }

    #[test]
    async fn test_put_get_fs() {
        // Given: some content.
        let content = create_content();

        // Given: a block store.
        let (_app, blockstore, path) =
            create_blockstore(format!("test-{}", std::thread::current().name().unwrap())).await;

        let test = async move {
            // When: we create a putter and write some content.
            let mut putter = blockstore.put(None);
            putter
                .write(content.as_slice(), CompressionAlgorithm::Uncompressed)
                .map_err(|e| anyhow::anyhow!("{e:?}"))?;

            // Then: the putter returns the appropriate root hash.
            let root = putter
                .finalize()
                .await
                .map_err(|e| anyhow::anyhow!("{e:?}"))?;
            let tree = hash_tree(content.as_slice());
            if root != Blake3Hash::from(tree.hash) {
                anyhow::bail!("invalid root hash");
            }

            // Then: our tree is stored as expected.
            let tree = hash_tree(content.as_slice());
            let shared = blockstore
                .get_tree(&Blake3Hash::from(tree.hash))
                .await
                .ok_or_else(|| anyhow::anyhow!("failed to get tree"))?;
            let tree2: &[[u8; 32]] = shared.as_ref().as_ref();
            assert_eq!(tree.tree, tree2, "tree is invalid");

            Ok(())
        };

        let result = test.await;

        if path.exists() {
            std::fs::remove_dir_all(path).unwrap();
        }

        result.expect("Test to pass");
    }

    #[tokio::test]
    async fn hash_consistency() {
        let (_app, blockstore, path) =
            create_blockstore(format!("test-{}", std::thread::current().name().unwrap())).await;

        const SIZE: usize = 4321;

        let expected_hash = blake3_tree::blake3::hash(&[0; SIZE]);

        let mut hasher = blake3_tree::blake3::tree::HashTreeBuilder::new();
        for _ in 0..SIZE {
            hasher.update(&[0; 1]);
        }
        let output = hasher.finalize();
        assert_eq!(output.hash, expected_hash);

        let mut putter = blockstore.put(None);
        putter
            .write(&[0; SIZE], CompressionAlgorithm::Uncompressed)
            .expect("failed to write to putter");
        let hash = putter.finalize().await.unwrap();
        assert_eq!(&hash, output.hash.as_bytes());

        if path.exists() {
            std::fs::remove_dir_all(path).unwrap();
        }
    }
}
