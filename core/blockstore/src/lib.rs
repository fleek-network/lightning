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
        NotifierInterface,
        SignerInterface,
        SyncQueryRunnerInterface,
        WithStartAndShutdown,
    };
    use lightning_notifier::Notifier;
    use lightning_signer::{Config as SignerConfig, Signer};
    use lightning_test_utils::consensus::{Config as ConsensusConfig, MockConsensus};
    use tokio::sync::mpsc;
    use tokio::test;

    use crate::blockstore::{Blockstore, BLOCK_SIZE};
    use crate::config::Config;

    struct AppState {
        app: Application<TestBinding>,
        signer: Signer<TestBinding>,
        _consensus: MockConsensus<TestBinding>,
        blockstore: Blockstore<TestBinding>,
        temp_dir_path: PathBuf,
    }

    impl Drop for AppState {
        fn drop(&mut self) {
            if self.temp_dir_path.exists() {
                std::fs::remove_dir_all(self.temp_dir_path.as_path()).unwrap();
            }
        }
    }

    partial!(TestBinding {
        ApplicationInterface = Application<Self>;
        BlockStoreInterface = Blockstore<Self>;
        SignerInterface = Signer<Self>;
        ConsensusInterface = MockConsensus<Self>;
        IndexerInterface = Indexer<Self>;
        NotifierInterface = Notifier<Self>;
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

    async fn create_app_state(test_name: String) -> AppState {
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

        let (update_socket, query_runner) =
            (app.transaction_executor(), app.sync_query(file!(), line!()));

        let mut signer =
            Signer::<TestBinding>::init(signer_config, query_runner.my_clone(file!(), line!()))
                .unwrap();

        let notifier = Notifier::<TestBinding>::init(&app);

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
            query_runner.my_clone(file!(), line!()),
            infusion::Blank::default(),
            None,
            &notifier,
        )
        .unwrap();

        signer.provide_mempool(consensus.mempool());

        let (new_block_tx, new_block_rx) = mpsc::channel(10);

        signer.provide_new_block_notify(new_block_rx);
        notifier.notify_on_new_block(new_block_tx);

        let indexer =
            Indexer::<TestBinding>::init(Default::default(), query_runner, &signer).unwrap();
        blockstore.provide_indexer(indexer);

        signer.start().await;
        consensus.start().await;

        AppState {
            app,
            signer,
            _consensus: consensus,
            blockstore,
            temp_dir_path: path,
        }
    }

    #[test]
    async fn test_put_verify() {
        // Given: some content.
        let content = create_content();
        // Given: app state with a blockstore.
        let state =
            create_app_state(format!("test-{}", std::thread::current().name().unwrap())).await;

        // Given: we put the content in the block store.
        let mut putter = state.blockstore.put(None);
        putter
            .write(content.as_slice(), CompressionAlgorithm::Uncompressed)
            .unwrap();
        putter.finalize().await.unwrap();

        // Given: the full tree.
        let hash_tree = hash_tree(content.as_slice());

        // When: we put the content by block and feed the proof to verify it.
        let mut putter = state.blockstore.put(Some(Blake3Hash::from(hash_tree.hash)));
        for (i, block) in content.chunks(BLOCK_SIZE).enumerate() {
            let proof = new_proof(&hash_tree.tree, i);
            putter.feed_proof(proof.as_slice()).unwrap();
            putter
                .write(block, CompressionAlgorithm::Uncompressed)
                .unwrap();
        }

        // Then: the putter returns the appropriate root hash and no errors.
        let root = putter.finalize().await.unwrap();
        if root != Blake3Hash::from(hash_tree.hash) {
            panic!("invalid root hash");
        }
    }

    #[test]
    async fn test_put_verify_invalid_content() {
        // Given: some content.
        let mut content = create_content();

        // Given: app state with a blockstore.
        let state =
            create_app_state(format!("test-{}", std::thread::current().name().unwrap())).await;

        // Given: we put the content in the block store and feed the proof to verify it.
        let mut putter = state.blockstore.put(None);
        putter
            .write(content.as_slice(), CompressionAlgorithm::Uncompressed)
            .unwrap();
        putter.finalize().await.unwrap();

        // Given: the full tree.
        let hash_tree = hash_tree(content.as_slice());

        // Given: make a change to the content.
        content[10] = 69;

        // When: we put a block with modified content and feed the proof to verify it.
        let mut putter = state.blockstore.put(Some(Blake3Hash::from(hash_tree.hash)));
        let mut blocks = content.chunks(BLOCK_SIZE);
        let proof = ProofBuf::new(&hash_tree.tree, 0);
        putter.feed_proof(proof.as_slice()).unwrap();

        // Then: write fails because content is invalid.
        assert!(
            putter
                .write(blocks.next().unwrap(), CompressionAlgorithm::Uncompressed)
                .is_err()
        );
    }

    #[test]
    async fn test_put_verify_one_chunk() {
        // Given: some content.
        let content = [0; BLOCK_SIZE];

        // Given: app state with a blockstore.
        let state =
            create_app_state(format!("test-{}", std::thread::current().name().unwrap())).await;

        // Given: we put the content in the block store.
        let mut putter = state.blockstore.put(None);
        putter
            .write(&content, CompressionAlgorithm::Uncompressed)
            .unwrap();
        putter.finalize().await.unwrap();

        // Given: the full tree.
        let hash_tree = hash_tree(&content);

        // When: we put one block and feed the proof to verify it.
        let mut putter = state.blockstore.put(Some(Blake3Hash::from(hash_tree.hash)));
        let proof = new_proof(&hash_tree.tree, 0);
        putter.feed_proof(proof.as_slice()).unwrap();
        putter
            .write(&content, CompressionAlgorithm::Uncompressed)
            .unwrap();

        // Then: the putter returns the appropriate root hash and no errors.
        let root = putter.finalize().await.unwrap();
        if root != Blake3Hash::from(hash_tree.hash) {
            panic!("Invalid root")
        }
    }

    #[test]
    async fn test_put_verify_one_chunk_small() {
        // Given: some content.
        let content = [0; 256];
        // Given: app state with a blockstore.
        let state =
            create_app_state(format!("test-{}", std::thread::current().name().unwrap())).await;

        // Given: we put the content in the block store.
        let mut putter = state.blockstore.put(None);
        putter
            .write(&content, CompressionAlgorithm::Uncompressed)
            .unwrap();
        putter.finalize().await.unwrap();

        // Given: the full tree.
        let hash_tree = hash_tree(&content);

        // When: we put one block and feed the proof to verify it.
        let mut putter = state.blockstore.put(Some(Blake3Hash::from(hash_tree.hash)));
        let proof = new_proof(&hash_tree.tree, 0);
        putter.feed_proof(proof.as_slice()).unwrap();
        putter
            .write(&content, CompressionAlgorithm::Uncompressed)
            .unwrap();

        // Then: the putter returns the appropriate root hash and no errors.
        let root = putter.finalize().await.unwrap();
        if root != Blake3Hash::from(hash_tree.hash) {
            panic!("Invalid root")
        }
    }

    #[test]
    async fn test_put_verify_one_chunk_and_a_half() {
        // Given: one chunk and another chunk smaller than a Blake3 chunk.
        let content = vec![vec![0; 256 * 1024], vec![1; 256]]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();

        // Given: app state with a blockstore.
        let state =
            create_app_state(format!("test-{}", std::thread::current().name().unwrap())).await;

        // Given: we put the content in the block store.
        let mut putter = state.blockstore.put(None);
        putter
            .write(&content, CompressionAlgorithm::Uncompressed)
            .unwrap();
        putter.finalize().await.unwrap();

        // Given: the full tree.
        let hash_tree = hash_tree(&content);

        // When: we put the content by block and feed the proof to verify it.
        let size = state
            .blockstore
            .read_all_to_vec(hash_tree.hash.as_bytes())
            .await
            .unwrap()
            .len();
        assert_eq!(size, 262400);

        // When: we verify the content.
        let mut putter = state.blockstore.put(Some(Blake3Hash::from(hash_tree.hash)));
        let proof = new_proof(&hash_tree.tree, 0);
        putter.feed_proof(proof.as_slice()).unwrap();
        putter
            .write(&content, CompressionAlgorithm::Uncompressed)
            .unwrap();

        // Then: the putter returns the appropriate root hash and no errors.
        let root = putter.finalize().await.unwrap();

        if root != Blake3Hash::from(hash_tree.hash) {
            panic!("Invalid root")
        }
    }

    #[test]
    async fn test_put_get_fs() {
        // Given: some content.
        let content = create_content();

        // Given: app state with a blockstore.
        let state =
            create_app_state(format!("test-{}", std::thread::current().name().unwrap())).await;

        // When: we create a putter and write some content.
        let mut putter = state.blockstore.put(None);
        putter
            .write(content.as_slice(), CompressionAlgorithm::Uncompressed)
            .unwrap();

        // Then: the putter returns the appropriate root hash.
        let root = putter.finalize().await.unwrap();
        let tree = hash_tree(content.as_slice());
        if root != Blake3Hash::from(tree.hash) {
            panic!("invalid root hash");
        }

        // Then: content registry was updated.
        let (_, sk) = state.signer.get_sk();
        let query_runner = state.app.sync_query(file!(), line!());
        let local_index = query_runner.pubkey_to_index(&sk.to_pk()).unwrap();

        let mut interval = tokio::time::interval(Duration::from_secs(1));
        loop {
            tokio::select! {
                _ = interval.tick() => {
                    if let Some(content_registry)
                        = query_runner.get_content_registry(&local_index) {
                        assert!(content_registry.contains(&root));
                        assert_eq!(content_registry.len(), 1);

                        let providers = query_runner
                            .get_cid_providers(&root).unwrap();
                        if !providers.is_empty() {
                            assert!(providers.contains(&local_index));
                            assert_eq!(providers.len(), 1);
                            break;
                        }
                    }
                }
            }
        }

        // Then: our tree is stored as expected.
        let tree = hash_tree(content.as_slice());
        let shared = state
            .blockstore
            .get_tree(&Blake3Hash::from(tree.hash))
            .await
            .ok_or_else(|| anyhow::anyhow!("failed to get tree"))
            .unwrap();
        let tree2: &[[u8; 32]] = shared.as_ref().as_ref();
        assert_eq!(tree.tree, tree2, "tree is invalid");
    }

    #[tokio::test]
    async fn hash_consistency() {
        let state =
            create_app_state(format!("test-{}", std::thread::current().name().unwrap())).await;

        const SIZE: usize = 4321;

        let expected_hash = blake3_tree::blake3::hash(&[0; SIZE]);

        let mut hasher = blake3_tree::blake3::tree::HashTreeBuilder::new();
        for _ in 0..SIZE {
            hasher.update(&[0; 1]);
        }
        let output = hasher.finalize();
        assert_eq!(output.hash, expected_hash);

        let mut putter = state.blockstore.put(None);
        putter
            .write(&[0; SIZE], CompressionAlgorithm::Uncompressed)
            .expect("failed to write to putter");
        let hash = putter.finalize().await.unwrap();
        assert_eq!(&hash, output.hash.as_bytes());
    }
}
