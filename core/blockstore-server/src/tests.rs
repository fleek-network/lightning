use std::borrow::Cow;
use std::collections::VecDeque;
use std::time::Duration;

use b3fs::bucket::file::uwriter::UntrustedFileWriter;
use b3fs::bucket::file::writer::FileWriter;
use b3fs::entry::{BorrowedEntry, BorrowedLink, OwnedEntry, OwnedLink};
use fleek_crypto::{AccountOwnerSecretKey, NodePublicKey, SecretKey};
use lightning_application::app::Application;
use lightning_application::config::ApplicationConfig;
use lightning_blockstore::blockstore::{Blockstore, BLOCK_SIZE};
use lightning_blockstore::config::Config as BlockstoreConfig;
use lightning_indexer::Indexer;
use lightning_interfaces::prelude::*;
use lightning_interfaces::types::{Genesis, GenesisNode, NodePorts, ServerRequest};
use lightning_interfaces::{DirTrustedWriter, _FileTrustedWriter};
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
use crate::blockstore_server::{FileFrame, Frame};
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
    let bucket = peers[0].blockstore().get_bucket();
    let mut writer_sender = FileWriter::new(&bucket).await.unwrap();
    writer_sender.write(content.as_slice()).await.unwrap();
    let root_hash = writer_sender.commit().await.unwrap();

    let mut network_wire = VecDeque::new();

    // The sender sends the content with the proofs to the receiver
    if let Ok(tree) = peers[0].blockstore().get_bucket().get(&root_hash).await {
        let num_blocks = tree.blocks();
        let mut reader = tree.into_file().unwrap().hashtree().await.unwrap();
        for block in 0..num_blocks {
            let hash = reader.get_hash(block).await.unwrap().unwrap();

            let proof = reader.generate_proof(block).await.unwrap();
            if !proof.is_empty() {
                let slice = proof.as_slice().to_owned();
                network_wire.push_back(Frame::File(FileFrame::Proof(Cow::Owned(slice))));
            }

            let chunk = peers[0]
                .blockstore()
                .get_bucket()
                .get_block_content(&hash)
                .await
                .unwrap()
                .unwrap();

            let frame = if block == num_blocks - 1 {
                Frame::File(FileFrame::LastChunk(Cow::Owned(chunk.clone())))
            } else {
                Frame::File(FileFrame::Chunk(Cow::Owned(chunk.clone())))
            };
            network_wire.push_back(frame);
        }
        network_wire.push_back(Frame::File(FileFrame::Eos));
    }

    // The receiver reads the frames and puts them into its blockstore
    let bucket = peers[1].blockstore().get_bucket();
    let mut putter = UntrustedFileWriter::new(&bucket, root_hash).await.unwrap();
    while let Some(frame) = network_wire.pop_front() {
        match frame {
            Frame::File(FileFrame::Proof(proof)) => putter.feed_proof(&proof).await.unwrap(),
            Frame::File(FileFrame::LastChunk(chunk)) => {
                putter.write(&chunk, true).await.unwrap();
            },
            Frame::File(FileFrame::Chunk(chunk)) => putter.write(&chunk, false).await.unwrap(),
            Frame::File(FileFrame::Eos) => {
                let hash = putter.commit().await.unwrap();
                assert_eq!(hash, root_hash);
                break;
            },
            Frame::Dir(_) => {
                panic!("imposible");
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
    let mut putter = peers[0].blockstore().file_writer().await.unwrap();
    putter.write(&content, true).await.unwrap();
    let hash = putter.commit().await.unwrap();

    // Send a request from peer 2 to peer 1
    let socket = peers[1].blockstore_server().get_socket();
    let mut res = socket
        .run(ServerRequest {
            hash,
            peer: node_index1,
        })
        .await
        .expect("Failed to send request");
    let result = res.recv().await.unwrap();
    match result {
        Ok(()) => {
            let recv_content = peers[1].blockstore().read_all_to_vec(&hash).await.unwrap();
            assert_eq!(recv_content, content);
        },
        Err(e) => {
            panic!("Failed to receive content: {e:?}");
        },
    }

    for mut peer in peers {
        peer.inner.shutdown().await;
        drop(peer);
    }
}

#[tokio::test]
async fn test_send_and_receive_dir() {
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

    let file1 = std::fs::read("/home/matthias/Desktop/testdir/file1").unwrap();
    let file2 = std::fs::read("/home/matthias/Desktop/testdir/file2").unwrap();
    let file3 = std::fs::read("/home/matthias/Desktop/testdir/subdir/file3").unwrap();

    // Put some data into the blockstore of peer 1
    let mut fputter = peers[0].blockstore().file_writer().await.unwrap();
    fputter.write(&file1, true).await.unwrap();
    let file1_hash = fputter.commit().await.unwrap();

    let mut fputter = peers[0].blockstore().file_writer().await.unwrap();
    fputter.write(&file2, true).await.unwrap();
    let file2_hash = fputter.commit().await.unwrap();

    let mut fputter = peers[0].blockstore().file_writer().await.unwrap();
    fputter.write(&file3, true).await.unwrap();
    let file3_hash = fputter.commit().await.unwrap();
    let entry = BorrowedEntry {
        name: "file3".as_bytes(),
        link: BorrowedLink::Content(&file3_hash),
    };
    let mut dputter_subdir = peers[0].blockstore().dir_writer(1).await.unwrap();
    dputter_subdir.insert(entry).await.unwrap();
    let subdir_hash = dputter_subdir.commit().await.unwrap();

    //let mut dputter = peers[0].blockstore().dir_writer(3).await.unwrap();
    let mut dputter = peers[0].blockstore().dir_writer(2).await.unwrap();

    let entry1 = BorrowedEntry {
        name: "file1".as_bytes(),
        link: BorrowedLink::Content(&file1_hash),
    };
    let entry2 = BorrowedEntry {
        name: "file2".as_bytes(),
        link: BorrowedLink::Content(&file2_hash),
    };
    let entry3 = BorrowedEntry {
        name: "subdir".as_bytes(),
        link: BorrowedLink::Content(&subdir_hash),
    };
    dputter.insert(entry1).await.unwrap();
    dputter.insert(entry2).await.unwrap();
    //dputter.insert(entry3).await.unwrap();

    // Test commit()
    let root_hash = dputter.commit().await.unwrap();

    // Send a request from peer 2 to peer 1
    let socket = peers[1].blockstore_server().get_socket();
    let mut res = socket
        .run(ServerRequest {
            hash: root_hash,
            peer: node_index1,
        })
        .await
        .expect("Failed to send request");
    let result = res.recv().await.unwrap();
    match result {
        Ok(()) => {
            let recv_content = peers[1]
                .blockstore()
                .read_all_to_vec(&root_hash)
                .await
                .unwrap();
            //assert_eq!(recv_content, content);
        },
        Err(e) => {
            panic!("Failed to receive content: {e:?}");
        },
    }

    for mut peer in peers {
        peer.inner.shutdown().await;
        drop(peer);
    }
}
