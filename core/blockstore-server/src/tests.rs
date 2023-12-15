use std::borrow::Cow;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::time::Duration;

use blake3_tree::ProofBuf;
use fleek_crypto::{
    AccountOwnerSecretKey,
    ConsensusSecretKey,
    NodePublicKey,
    NodeSecretKey,
    SecretKey,
};
use lightning_application::app::Application;
use lightning_application::config::{Config as AppConfig, Mode, StorageConfig};
use lightning_application::genesis::{Genesis, GenesisNode};
use lightning_blockstore::blockstore::{Blockstore, BLOCK_SIZE};
use lightning_blockstore::config::Config as BlockstoreConfig;
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::types::{
    CompressionAlgoSet,
    CompressionAlgorithm,
    NodePorts,
    ServerRequest,
};
use lightning_interfaces::{
    partial,
    ApplicationInterface,
    BlockStoreInterface,
    BlockStoreServerInterface,
    IncrementalPutInterface,
    NotifierInterface,
    PoolInterface,
    ReputationAggregatorInterface,
    SignerInterface,
    SyncQueryRunnerInterface,
    TopologyInterface,
    WithStartAndShutdown,
};
use lightning_notifier::Notifier;
use lightning_pool::{muxer, Config as PoolConfig, Pool};
use lightning_rep_collector::ReputationAggregator;
use lightning_signer::{utils, Config as SignerConfig, Signer};
use lightning_topology::{Config as TopologyConfig, Topology};

use super::BlockStoreServer;
use crate::blockstore_server::Frame;
use crate::config::Config;

partial!(TestBinding {
    BlockStoreInterface = Blockstore<Self>;
    BlockStoreServerInterface = BlockStoreServer<Self>;
    ApplicationInterface = Application<Self>;
    PoolInterface = Pool<Self>;
    SignerInterface = Signer<Self>;
    NotifierInterface = Notifier<Self>;
    TopologyInterface = Topology<Self>;
    ReputationAggregatorInterface = ReputationAggregator<Self>;
});

fn create_content() -> Vec<u8> {
    (0..4)
        .map(|i| Vec::from([i; BLOCK_SIZE]))
        .flat_map(|a| a.into_iter())
        .collect()
}

struct Peer<C: Collection> {
    pool: C::PoolInterface,
    blockstore: Blockstore<C>,
    blockstore_server: C::BlockStoreServerInterface,
    _rep_aggregator: C::ReputationAggregatorInterface,
    node_public_key: NodePublicKey,
}

async fn get_peers(
    test_name: &str,
    port_offset: u16,
    num_peers: usize,
) -> (Vec<Peer<TestBinding>>, Application<TestBinding>, PathBuf) {
    let mut signers_configs = Vec::new();
    let mut genesis = Genesis::load().unwrap();
    let path = std::env::temp_dir()
        .join("lightning-pool-test")
        .join(test_name);
    if path.exists() {
        std::fs::remove_dir_all(&path).unwrap();
    }
    let owner_secret_key = AccountOwnerSecretKey::generate();
    let owner_public_key = owner_secret_key.to_pk();

    genesis.node_info = vec![];
    for i in 0..num_peers {
        let node_secret_key = NodeSecretKey::generate();
        let consensus_secret_key = ConsensusSecretKey::generate();
        let node_key_path = path.join(format!("node{i}/node.pem"));
        let consensus_key_path = path.join(format!("node{i}/cons.pem"));
        utils::save(&node_key_path, node_secret_key.encode_pem()).unwrap();
        utils::save(&consensus_key_path, consensus_secret_key.encode_pem()).unwrap();
        let signer_config = SignerConfig {
            node_key_path: node_key_path.try_into().unwrap(),
            consensus_key_path: consensus_key_path.try_into().unwrap(),
        };
        signers_configs.push(signer_config);

        genesis.node_info.push(GenesisNode::new(
            owner_public_key.into(),
            node_secret_key.to_pk(),
            "127.0.0.1".parse().unwrap(),
            consensus_secret_key.to_pk(),
            "127.0.0.1".parse().unwrap(),
            node_secret_key.to_pk(),
            NodePorts {
                primary: 48000_u16,
                worker: 48101_u16,
                mempool: 48202_u16,
                rpc: 48300_u16,
                pool: port_offset + i as u16,
                pinger: 48600_u16,
                handshake: Default::default(),
            },
            None,
            true,
        ));
    }

    let blockstore_config = BlockstoreConfig {
        root: path.join("dummy-blockstore").try_into().unwrap(),
    };
    let dummy_blockstore = Blockstore::init(blockstore_config).unwrap();
    let app = Application::<TestBinding>::init(
        AppConfig {
            genesis: Some(genesis),
            mode: Mode::Test,
            testnet: false,
            storage: StorageConfig::InMemory,
            db_path: None,
            db_options: None,
        },
        dummy_blockstore,
    )
    .unwrap();
    app.start().await;

    let mut peers = Vec::new();
    for (i, signer_config) in signers_configs.into_iter().enumerate() {
        let (_, query_runner) = (app.transaction_executor(), app.sync_query());
        let signer = Signer::<TestBinding>::init(signer_config, query_runner.clone()).unwrap();
        let notifier = Notifier::<TestBinding>::init(&app);
        let topology = Topology::<TestBinding>::init(
            TopologyConfig::default(),
            signer.get_ed25519_pk(),
            query_runner.clone(),
        )
        .unwrap();
        let rep_aggregator = ReputationAggregator::<TestBinding>::init(
            Default::default(),
            signer.get_socket(),
            notifier.clone(),
            query_runner.clone(),
        )
        .unwrap();

        let config = PoolConfig {
            max_idle_timeout: Duration::from_secs(5),
            address: format!("0.0.0.0:{}", port_offset + i as u16)
                .parse()
                .unwrap(),
            ..Default::default()
        };
        let pool = Pool::<TestBinding, muxer::quinn::QuinnMuxer>::init(
            config,
            &signer,
            query_runner,
            notifier,
            topology,
            rep_aggregator.get_reporter(),
        )
        .unwrap();

        let blockstore_config = BlockstoreConfig {
            root: path.join(format!("node{i}/blockstore")).try_into().unwrap(),
        };
        let blockstore = Blockstore::init(blockstore_config).unwrap();

        let bs_config = Config {
            max_conc_req: 10,
            max_conc_res: 10,
        };
        let blockstore_server = BlockStoreServer::<TestBinding>::init(
            bs_config,
            blockstore.clone(),
            &pool,
            rep_aggregator.get_reporter(),
        )
        .unwrap();

        let peer = Peer::<TestBinding> {
            pool,
            blockstore,
            blockstore_server,
            _rep_aggregator: rep_aggregator,
            node_public_key: signer.get_ed25519_pk(),
        };
        peers.push(peer);
    }
    (peers, app, path)
}

/// Temporary sanity check on the flow
#[tokio::test]
async fn test_stream_verified_content() {
    let path1 = std::env::temp_dir().join("lightning-blockstore-transfer-1");
    let path2 = std::env::temp_dir().join("lightning-blockstore-transfer-2");

    let blockstore1 = Blockstore::<TestBinding>::init(BlockstoreConfig {
        root: path1.clone().try_into().unwrap(),
    })
    .unwrap();

    let blockstore2 = Blockstore::<TestBinding>::init(BlockstoreConfig {
        root: path2.clone().try_into().unwrap(),
    })
    .unwrap();

    let content = create_content();

    // Put some content into the sender's blockstore
    let mut putter = blockstore1.put(None);
    putter
        .write(content.as_slice(), CompressionAlgorithm::Uncompressed)
        .unwrap();
    let root_hash = putter.finalize().await.unwrap();

    let mut network_wire = VecDeque::new();

    // The sender sends the content with the proofs to the receiver
    if let Some(tree) = blockstore1.get_tree(&root_hash).await {
        for block in 0..tree.len() {
            let compr = CompressionAlgoSet::default(); // rustfmt
            let chunk = blockstore1
                .get(block as u32, &tree[block], compr)
                .await
                .expect("failed to get block from store");
            let proof = if block == 0 {
                ProofBuf::new(tree.as_ref().as_ref(), 0)
            } else {
                ProofBuf::resume(tree.as_ref().as_ref(), block)
            };

            if !proof.is_empty() {
                network_wire.push_back(Frame::Proof(Cow::Owned(proof.as_slice().to_vec())));
            }
            network_wire.push_back(Frame::Chunk(Cow::Owned(chunk.content.clone())));
        }
        network_wire.push_back(Frame::Eos);
    }

    // The receiver reads the frames and puts them into its blockstore
    let mut putter = blockstore2.put(Some(root_hash));
    while let Some(frame) = network_wire.pop_front() {
        match frame {
            Frame::Proof(proof) => putter.feed_proof(&proof).unwrap(),
            Frame::Chunk(chunk) => putter
                .write(&chunk, CompressionAlgorithm::Uncompressed)
                .unwrap(),
            Frame::Eos => {
                let hash = putter.finalize().await.unwrap();
                assert_eq!(hash, root_hash);
                break;
            },
        }
    }

    // Make sure the content matches
    let content1 = blockstore1.read_all_to_vec(&root_hash).await;
    let content2 = blockstore2.read_all_to_vec(&root_hash).await;
    assert_eq!(content1, content2);

    // Clean up test
    if path1.exists() {
        std::fs::remove_dir_all(path1).unwrap();
    }
    if path2.exists() {
        std::fs::remove_dir_all(path2).unwrap();
    }
}

#[tokio::test]
async fn test_send_and_receive() {
    let (peers, app, path) = get_peers("send_and_receive", 49200, 2).await;
    let query_runner = app.sync_query();
    for peer in &peers {
        peer.pool.start().await;
        peer.blockstore_server.start().await;
    }
    tokio::time::sleep(Duration::from_millis(500)).await;

    let node_index1 = query_runner
        .pubkey_to_index(&peers[0].node_public_key)
        .unwrap();

    let content = create_content();
    // Put some data into the blockstore of peer 1
    let mut putter = peers[0].blockstore.put(None);
    putter
        .write(&content, CompressionAlgorithm::Uncompressed)
        .unwrap();
    let hash = putter.finalize().await.unwrap();

    // Send a request from peer 2 to peer 1
    let socket = peers[1].blockstore_server.get_socket();
    let mut res = socket
        .run(ServerRequest {
            hash,
            peer: node_index1,
        })
        .await
        .expect("Failed to send request");
    match res.recv().await.unwrap() {
        Ok(()) => {
            let recv_content = peers[1].blockstore.read_all_to_vec(&hash).await.unwrap();
            assert_eq!(recv_content, content);
        },
        Err(e) => panic!("Failed to receive content: {e:?}"),
    }

    for peer in &peers {
        peer.pool.shutdown().await;
        peer.blockstore_server.shutdown().await;
    }

    // Clean up test
    if path.exists() {
        std::fs::remove_dir_all(path).unwrap();
    }
}
