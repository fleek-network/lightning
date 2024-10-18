use std::borrow::Cow;
use std::collections::VecDeque;
use std::time::Duration;

use blake3_tree::ProofBuf;
use fleek_crypto::{AccountOwnerSecretKey, NodePublicKey, SecretKey};
use lightning_application::app::Application;
use lightning_application::config::ApplicationConfig;
use lightning_blockstore::blockstore::{Blockstore, BLOCK_SIZE};
use lightning_blockstore::config::Config as BlockstoreConfig;
use lightning_indexer::Indexer;
use lightning_interfaces::prelude::*;
use lightning_interfaces::types::{
    CompressionAlgoSet,
    CompressionAlgorithm,
    Genesis,
    GenesisNode,
    NodePorts,
    ServerRequest,
};
use lightning_node::Node;
use lightning_notifier::Notifier;
use lightning_pool::{Config as PoolConfig, PoolProvider};
use lightning_rep_collector::ReputationAggregator;
use lightning_signer::Signer;
use lightning_test_utils::json_config::JsonConfigProvider;
use lightning_test_utils::keys::EphemeralKeystore;
use lightning_topology::Topology;
use tempfile::{tempdir, TempDir};

use super::BlockstoreServer;
use crate::blockstore_server::Frame;
use crate::config::Config;

partial_node_components!(TestBinding {
    ConfigProviderInterface = JsonConfigProvider;
    BlockstoreInterface = Blockstore<Self>;
    BlockstoreServerInterface = BlockstoreServer<Self>;
    ApplicationInterface = Application<Self>;
    PoolInterface = PoolProvider<Self>;
    KeystoreInterface = EphemeralKeystore<Self>;
    SignerInterface = Signer<Self>;
    NotifierInterface = Notifier<Self>;
    TopologyInterface = Topology<Self>;
    ReputationAggregatorInterface = ReputationAggregator<Self>;
    IndexerInterface = Indexer<Self>;
});

fn create_content() -> Vec<u8> {
    (0..4)
        .map(|i| Vec::from([i; BLOCK_SIZE]))
        .flat_map(|a| a.into_iter())
        .collect()
}

struct Peer<C: NodeComponents> {
    inner: Node<C>,
    node_public_key: NodePublicKey,
}

impl<C: NodeComponents> Peer<C> {
    fn blockstore(&self) -> fdi::Ref<C::BlockstoreInterface> {
        self.inner.provider.get::<C::BlockstoreInterface>()
    }

    fn app(&self) -> fdi::Ref<C::ApplicationInterface> {
        self.inner.provider.get::<C::ApplicationInterface>()
    }

    fn blockstore_server(&self) -> fdi::Ref<C::BlockstoreServerInterface> {
        self.inner.provider.get::<C::BlockstoreServerInterface>()
    }
}

async fn get_peers(
    temp_dir: &TempDir,
    port_offset: u16,
    num_peers: usize,
) -> Vec<Peer<TestBinding>> {
    let mut keystores = Vec::new();
    let mut genesis = Genesis::default();
    let owner_secret_key = AccountOwnerSecretKey::generate();
    let owner_public_key = owner_secret_key.to_pk();

    genesis.topology_target_k = 8;
    genesis.topology_min_nodes = 16;

    genesis.node_info = vec![];
    for i in 0..num_peers {
        let keystore = EphemeralKeystore::<TestBinding>::default();
        let (consensus_secret_key, node_secret_key) =
            (keystore.get_bls_sk(), keystore.get_ed25519_sk());
        keystores.push(keystore);

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

    let genesis_path = genesis
        .write_to_dir(temp_dir.path().to_path_buf().try_into().unwrap())
        .unwrap();

    let mut peers = Vec::new();
    for (i, keystore) in keystores.into_iter().enumerate() {
        let node_public_key = keystore.get_ed25519_pk();
        let node = Node::<TestBinding>::init_with_provider(
            lightning_interfaces::fdi::Provider::default()
                .with(
                    JsonConfigProvider::default()
                        .with::<Application<TestBinding>>(ApplicationConfig::test(
                            genesis_path.clone(),
                        ))
                        .with::<PoolProvider<TestBinding>>(PoolConfig {
                            max_idle_timeout: Duration::from_secs(5),
                            address: format!("0.0.0.0:{}", port_offset + i as u16)
                                .parse()
                                .unwrap(),
                            ..Default::default()
                        })
                        .with::<Blockstore<TestBinding>>(BlockstoreConfig {
                            root: temp_dir
                                .path()
                                .join(format!("node{i}/blockstore"))
                                .try_into()
                                .unwrap(),
                        })
                        .with::<BlockstoreServer<TestBinding>>(Config {
                            max_conc_req: 10,
                            max_conc_res: 10,
                        }),
                )
                .with(keystore.clone()),
        )
        .unwrap();

        let peer = Peer::<TestBinding> {
            inner: node,
            node_public_key,
        };
        peers.push(peer);
    }
    peers
}

/// Temporary sanity check on the flow
#[tokio::test]
async fn test_stream_verified_content() {
    let temp_dir = tempdir().unwrap();
    let peers = get_peers(&temp_dir, 49200, 2).await;

    let content = create_content();

    // Put some content into the sender's blockstore
    let mut putter = peers[0].blockstore().put(None);
    putter
        .write(content.as_slice(), CompressionAlgorithm::Uncompressed)
        .unwrap();
    let root_hash = putter.finalize().await.unwrap();

    let mut network_wire = VecDeque::new();

    // The sender sends the content with the proofs to the receiver
    if let Some(tree) = peers[0].blockstore().get_tree(&root_hash).await {
        for block in 0..tree.len() {
            let compr = CompressionAlgoSet::default(); // rustfmt
            let chunk = peers[0]
                .blockstore()
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
    let mut putter = peers[1].blockstore().put(Some(root_hash));
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
    let content1 = peers[0].blockstore().read_all_to_vec(&root_hash).await;
    let content2 = peers[1].blockstore().read_all_to_vec(&root_hash).await;
    assert_eq!(content1, content2);
}

#[tokio::test]
async fn test_send_and_receive() {
    let temp_dir = tempdir().unwrap();
    let peers = get_peers(&temp_dir, 49200, 2).await;
    let query_runner = peers[0].app().sync_query();
    for peer in &peers {
        peer.inner.start().await;
    }
    tokio::time::sleep(Duration::from_millis(500)).await;

    let node_index1 = query_runner
        .pubkey_to_index(&peers[0].node_public_key)
        .unwrap();

    let content = create_content();
    // Put some data into the blockstore of peer 1
    let mut putter = peers[0].blockstore().put(None);
    putter
        .write(&content, CompressionAlgorithm::Uncompressed)
        .unwrap();
    let hash = putter.finalize().await.unwrap();

    // Send a request from peer 2 to peer 1
    let socket = peers[1].blockstore_server().get_socket();
    let mut res = socket
        .run(ServerRequest {
            hash,
            peer: node_index1,
        })
        .await
        .expect("Failed to send request");
    match res.recv().await.unwrap() {
        Ok(()) => {
            let recv_content = peers[1].blockstore().read_all_to_vec(&hash).await.unwrap();
            assert_eq!(recv_content, content);
        },
        Err(e) => panic!("Failed to receive content: {e:?}"),
    }

    for mut peer in peers {
        peer.inner.shutdown().await;
        drop(peer);
    }
}
