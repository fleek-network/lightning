use std::collections::{HashMap, VecDeque};
use std::mem;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use blake3_tree::blake3::tree::HashTree;
use blake3_tree::ProofBuf;
use bytes::{Buf, BufMut, Bytes, BytesMut};
use infusion::c;
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::types::{
    Blake3Hash,
    CompressionAlgoSet,
    CompressionAlgorithm,
    NodeIndex,
};
use lightning_interfaces::{
    BlockStoreInterface,
    IncrementalPutInterface,
    PoolInterface,
    RejectReason,
    Request,
    Requester,
    Responder,
    Response,
    WithStartAndShutdown,
};
use log::error;
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, mpsc, oneshot, Notify};
use tokio::task::JoinSet;
use tokio::time::timeout;
use tokio_stream::StreamExt;

// TODO(matthias): This must be the same value as the `BLOCK_SIZE` constant defined in the block
// store implementation. We should define a single constant somewhere else.
const BLOCK_SIZE: usize = 256 << 10;
const REQUEST_TIMEOUT: Duration = Duration::from_millis(100);

pub struct BlockstoreServer<C: Collection> {
    inner: Arc<BlockstoreServerInner<C>>,
    is_running: Arc<AtomicBool>,
    shutdown_notify: Arc<Notify>,
}

impl<C: Collection> BlockstoreServer<C> {
    #[allow(unused)]
    pub fn init(
        blockstore: C::BlockStoreInterface,
        request_rx: mpsc::Receiver<ServerRequest>,
        max_conc_req: usize,
        max_conc_res: usize,
        pool_requester: c!(C::PoolInterface::Requester),
        pool_responder: c!(C::PoolInterface::Responder),
    ) -> Result<Self> {
        let shutdown_notify = Arc::new(Notify::new());

        let inner = BlockstoreServerInner::<C>::new(
            blockstore,
            request_rx,
            max_conc_req,
            max_conc_res,
            pool_requester,
            pool_responder,
            shutdown_notify.clone(),
        );

        Ok(Self {
            inner: Arc::new(inner),
            is_running: Arc::new(AtomicBool::new(false)),
            shutdown_notify,
        })
    }
}

#[async_trait]
impl<C: Collection> WithStartAndShutdown for BlockstoreServer<C> {
    /// Returns true if this system is running or not.
    fn is_running(&self) -> bool {
        self.is_running.load(Ordering::Relaxed)
    }

    /// Start the system, should not do anything if the system is already
    /// started.
    async fn start(&self) {
        if !self.is_running() {
            let inner = self.inner.clone();
            let is_running = self.is_running.clone();
            tokio::spawn(async move {
                inner.start().await;
                is_running.store(false, Ordering::Relaxed);
            });
            self.is_running.store(true, Ordering::Relaxed);
        } else {
            error!("Can not start blockstore server because it already running");
        }
    }

    /// Send the shutdown signal to the system.
    async fn shutdown(&self) {
        self.shutdown_notify.notify_one();
    }
}

#[allow(clippy::type_complexity)]
pub struct BlockstoreServerInner<C: Collection> {
    blockstore: C::BlockStoreInterface,
    request_rx: Arc<Mutex<Option<mpsc::Receiver<ServerRequest>>>>,
    max_conc_req: usize,
    max_conc_res: usize,
    num_responses: Arc<AtomicUsize>,
    pool_requester: c!(C::PoolInterface::Requester),
    pool_responder: Arc<Mutex<Option<c!(C::PoolInterface::Responder)>>>,
    shutdown_notify: Arc<Notify>,
}

impl<C: Collection> BlockstoreServerInner<C> {
    #[allow(unused)]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        blockstore: C::BlockStoreInterface,
        request_rx: mpsc::Receiver<ServerRequest>,
        max_conc_req: usize,
        max_conc_res: usize,
        pool_requester: c!(C::PoolInterface::Requester),
        pool_responder: c!(C::PoolInterface::Responder),
        shutdown_notify: Arc<Notify>,
    ) -> Self {
        Self {
            blockstore,
            request_rx: Arc::new(Mutex::new(Some(request_rx))),
            max_conc_req,
            max_conc_res,
            num_responses: Arc::new(AtomicUsize::new(0)),
            pool_requester,
            pool_responder: Arc::new(Mutex::new(Some(pool_responder))),
            shutdown_notify,
        }
    }

    pub async fn start(&self) {
        let mut pending_requests: HashMap<
            PeerRequest,
            broadcast::Sender<Result<(), PeerRequestError>>,
        > = HashMap::new();
        let mut request_rx = self.request_rx.lock().unwrap().take().unwrap();
        let mut pool_responder = self.pool_responder.lock().unwrap().take().unwrap();
        let mut tasks = JoinSet::new();
        let mut queue = VecDeque::new();
        loop {
            tokio::select! {
                _ = self.shutdown_notify.notified() => {
                    break;
                }
                Ok((request, responder)) = pool_responder.get_next_request() => {
                    // TODO(matthias): find out which peer the request came from
                    match PeerRequest::try_from(request) {
                        Ok(request) => {
                            if self.num_responses.load(Ordering::Relaxed) > self.max_conc_res {
                                responder.reject(RejectReason::TooManyRequests);
                            } else {
                                let blockstore = self.blockstore.clone();
                                let num_responses = self.num_responses.clone();
                                tokio::spawn(async move {
                                    handle_request::<C>(
                                        request,
                                        blockstore,
                                        responder,
                                        num_responses
                                    ).await;
                                });
                            }
                        }
                        Err(e) => error!("Failed to decode request from peer: {e:?}"),
                    }
                }
                Some(request) = request_rx.recv() => {
                    let peer_request = PeerRequest { hash: request.hash };
                    let rx = if let Some(tx) = pending_requests.get(&peer_request) {
                        // If a request for this hash is currently pending, subscribe to get
                        // notified about the result.
                        tx.subscribe()
                    } else {
                        // If no request for this hash currently exists, create new request.
                        if tasks.len() < self.max_conc_req {
                            let blockstore = self.blockstore.clone();
                            let pool_requester = self.pool_requester.clone();
                            let peer_request_ = peer_request.clone();
                            tasks.spawn(async move {
                                send_request::<C>(
                                    request.peer,
                                    peer_request_,
                                    blockstore,
                                    pool_requester
                                ).await
                            });
                        } else {
                            queue.push_back(peer_request.clone());
                        }
                        let (tx, rx) = broadcast::channel(1);
                        pending_requests.insert(peer_request, tx);
                        rx
                    };
                    request.response.send(rx).expect("Failed to send response");
                }
                Some(res) = tasks.join_next() => {
                    match res {
                        Ok(Ok(peer_request)) => {
                            if let Some(tx) = pending_requests.remove(&peer_request) {
                                tx.send(Ok(())).expect("Failed to send response");
                            }
                        },
                        Ok(Err(error_res)) => {
                            if let Some(tx) = pending_requests.remove(&error_res.request) {
                                tx.send(Err(error_res.error)).expect("Failed to send response");
                            }
                            error!("Failed to fetch data from peer");
                        },
                        Err(e) => error!("Failed to join task: {e:?}"),
                    }
                }
            }
        }
        *self.request_rx.lock().unwrap() = Some(request_rx);
        *self.pool_responder.lock().unwrap() = Some(pool_responder);
    }
}

#[derive(Serialize, Deserialize)]
enum Message {
    Request {
        hash: Blake3Hash,
        address: SocketAddr,
    },
    Response {
        data: Vec<u8>,
    },
}

#[derive(Debug)]
pub struct ServerRequest {
    pub hash: Blake3Hash,
    pub peer: NodeIndex,
    pub response: oneshot::Sender<broadcast::Receiver<Result<(), PeerRequestError>>>,
}

#[derive(Debug, Clone, thiserror::Error)]
#[error("Failed to fetch data from other peers")]
pub enum PeerRequestError {
    Timeout,
    Rejected,
    Incomplete,
}

#[derive(Debug, Hash, PartialEq, Eq, Clone)]
pub struct PeerRequest {
    hash: Blake3Hash,
    //block_counter: u32,
}

impl From<PeerRequest> for Bytes {
    fn from(value: PeerRequest) -> Self {
        let mut buf = BytesMut::with_capacity(value.hash.len());
        buf.put_slice(&value.hash);
        //buf.put_u32(value.block_counter);
        buf.into()
    }
}

impl TryFrom<Bytes> for PeerRequest {
    type Error = anyhow::Error;

    fn try_from(mut value: Bytes) -> Result<Self> {
        let hash_len = mem::size_of::<Blake3Hash>();
        if value.len() != hash_len {
            return Err(anyhow!("Number of bytes must be {}", hash_len));
        }
        let hash = value.split_to(hash_len);
        //let block_counter = value.get_u32();
        Ok(Self {
            hash: hash.to_vec().try_into().unwrap(),
            //block_counter,
        })
    }
}

pub enum Frame {
    Proof(Vec<u8>),
    Chunk(Vec<u8>),
    Eos,
}

impl From<Frame> for Bytes {
    fn from(value: Frame) -> Self {
        let mut b = BytesMut::new();
        match value {
            Frame::Proof(proof) => {
                b.put_u8(0x00);
                b.put_slice(&proof);
            },
            Frame::Chunk(chunk) => {
                b.put_u8(0x01);
                b.put_slice(&chunk);
            },
            Frame::Eos => {
                b.put_u8(0x02);
            },
        }
        b.freeze()
    }
}

impl TryFrom<Bytes> for Frame {
    type Error = anyhow::Error;

    fn try_from(mut value: Bytes) -> Result<Self> {
        match value.get_u8() {
            0x00 => Ok(Frame::Proof(value.to_vec())),
            0x01 => Ok(Frame::Chunk(value.to_vec())),
            0x02 => Ok(Frame::Eos),
            _ => Err(anyhow!("Unknown magic byte")),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub struct ErrorResponse {
    error: PeerRequestError,
    request: PeerRequest,
}

impl std::fmt::Display for ErrorResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

async fn handle_request<C: Collection>(
    peer_request: PeerRequest,
    blockstore: C::BlockStoreInterface,
    mut request: <c!(C::PoolInterface::Responder) as Responder>::Request,
    num_responses: Arc<AtomicUsize>,
) {
    num_responses.fetch_add(1, Ordering::Relaxed);
    if let Some(proof) = blockstore.get_tree(&peer_request.hash).await {
        let tree = HashTree {
            hash: peer_request.hash.into(),
            tree: proof.0.clone(),
        };
        let num_blocks = (tree.tree.len() + 1) / 2;
        let mut block = 0;
        let mut buffer = BytesMut::new();
        for i in 0_u32.. {
            let idx = (i * 2 - i.count_ones()) as usize;
            if idx < proof.0.len() {}
            let idx = if idx < proof.0.len() {
                idx
            } else {
                let _ = request.send(Bytes::from(Frame::Eos)).await;
                return;
            };
            let compr = CompressionAlgoSet::default(); // rustfmt
            let Some(chunk) = blockstore.get(i, &proof.0[idx], compr).await else {
                let _ = request.send(Bytes::from(Frame::Eos)).await;
                return;
            };
            buffer.put(chunk.content.as_slice());

            let mut proof = if block == 0 {
                ProofBuf::new(&tree.tree, 0)
            } else {
                ProofBuf::resume(&tree.tree, block)
            };

            while !buffer.is_empty() {
                if !proof.is_empty() {
                    let _ = request
                        .send(Bytes::from(Frame::Proof(proof.as_slice().to_vec())))
                        .await;
                };

                let bytes = buffer.split_to(buffer.len().min(BLOCK_SIZE));
                let _ = request
                    .send(Bytes::from(Frame::Chunk(bytes.to_vec())))
                    .await;
                block += 1;
                if block < num_blocks {
                    proof = ProofBuf::resume(&tree.tree, block)
                }
            }
        }
    }
    num_responses.fetch_sub(1, Ordering::Relaxed);
}

async fn send_request<C: Collection>(
    peer: NodeIndex,
    request: PeerRequest,
    blockstore: C::BlockStoreInterface,
    pool_requester: c!(C::PoolInterface::Requester),
) -> Result<PeerRequest, ErrorResponse> {
    match timeout(
        REQUEST_TIMEOUT,
        pool_requester.request(peer, Bytes::from(request.clone())),
    )
    .await
    {
        Ok(Ok(response)) => {
            match response.status_code() {
                Ok(()) => {
                    let mut body = response.body();
                    let mut putter = blockstore.put(Some(request.hash));
                    while let Some(bytes) = body.next().await {
                        let Ok(bytes) = bytes else {
                            return Err(ErrorResponse {
                                error: PeerRequestError::Incomplete,
                                request,
                            });
                        };
                        let Ok(frame) = Frame::try_from(bytes) else {
                            return Err(ErrorResponse {
                                error: PeerRequestError::Incomplete,
                                request,
                            });
                        };
                        match frame {
                            Frame::Proof(proof) => putter.feed_proof(&proof).unwrap(),
                            Frame::Chunk(chunk) => putter
                                .write(&chunk, CompressionAlgorithm::Uncompressed)
                                .unwrap(),
                            Frame::Eos => {
                                let _hash = putter.finalize().await.unwrap();
                                // TODO(matthias): do we have to compare this hash to the
                                // requested hash?
                                return Ok(request);
                            },
                        }
                    }
                    Err(ErrorResponse {
                        error: PeerRequestError::Incomplete,
                        request,
                    })
                },
                Err(_reason) => Err(ErrorResponse {
                    error: PeerRequestError::Rejected,
                    request,
                }),
            }
        },
        Ok(Err(_)) => Err(ErrorResponse {
            error: PeerRequestError::Incomplete,
            request,
        }),
        Err(_) => Err(ErrorResponse {
            error: PeerRequestError::Timeout,
            request,
        }),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::path::PathBuf;
    use std::time::Duration;

    use blake3_tree::blake3::tree::HashTree;
    use blake3_tree::ProofBuf;
    use bytes::{BufMut, BytesMut};
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
    use lightning_interfaces::types::{CompressionAlgoSet, CompressionAlgorithm, NodePorts};
    use lightning_interfaces::{
        partial,
        ApplicationInterface,
        BlockStoreInterface,
        IncrementalPutInterface,
        NotifierInterface,
        PoolInterface,
        ServiceScope,
        SignerInterface,
        SyncQueryRunnerInterface,
        TopologyInterface,
        WithStartAndShutdown,
    };
    use lightning_notifier::Notifier;
    use lightning_pool::{muxer, Config as PoolConfig, Pool};
    use lightning_signer::{utils, Config as SignerConfig, Signer};
    use lightning_topology::{Config as TopologyConfig, Topology};
    use tokio::sync::{mpsc, oneshot};

    use super::{BlockstoreServer, ServerRequest};
    use crate::blockstore_server::Frame;

    partial!(TestBinding {
        BlockStoreInterface = Blockstore<Self>;
        ApplicationInterface = Application<Self>;
        PoolInterface = Pool<Self>;
        SignerInterface = Signer<Self>;
        NotifierInterface = Notifier<Self>;
        TopologyInterface = Topology<Self>;
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
        blockstore_server: BlockstoreServer<C>,
        node_public_key: NodePublicKey,
        request_tx: mpsc::Sender<ServerRequest>,
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
                    dht: 48500_u16,
                    handshake: 48600_u16,
                    blockstore: 48700_u16,
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
            let config = PoolConfig {
                max_idle_timeout: 300,
                address: format!("0.0.0.0:{}", port_offset + i as u16)
                    .parse()
                    .unwrap(),
            };
            let pool = Pool::<TestBinding, muxer::quinn::QuinnMuxer>::init(
                config,
                &signer,
                query_runner,
                notifier,
                topology,
            )
            .unwrap();

            let blockstore_config = BlockstoreConfig {
                root: path.join(format!("node{i}/blockstore")).try_into().unwrap(),
            };
            let blockstore = Blockstore::init(blockstore_config).unwrap();

            let (pool_requester, pool_responder) =
                pool.open_req_res(ServiceScope::BlockstoreServer);
            let (request_tx, request_rx) = mpsc::channel(10);
            let blockstore_server = BlockstoreServer::<TestBinding>::init(
                blockstore.clone(),
                request_rx,
                10,
                10,
                pool_requester,
                pool_responder,
            )
            .unwrap();

            let peer = Peer::<TestBinding> {
                pool,
                blockstore,
                blockstore_server,
                node_public_key: signer.get_ed25519_pk(),
                request_tx,
            };
            peers.push(peer);
        }
        (peers, app, path)
    }

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
        if let Some(proof) = blockstore1.get_tree(&root_hash).await {
            let tree = HashTree {
                hash: root_hash.into(),
                tree: proof.0.clone(),
            };
            let num_blocks = (tree.tree.len() + 1) / 2;
            let mut block = 0;
            let mut buffer = BytesMut::new();
            for i in 0_u32.. {
                let idx = (i * 2 - i.count_ones()) as usize;
                if idx < proof.0.len() {}
                let idx = if idx < proof.0.len() {
                    idx
                } else {
                    network_wire.push_back(Frame::Eos);
                    break;
                };
                let compr = CompressionAlgoSet::default();
                let Some(chunk) = blockstore1.get(i, &proof.0[idx], compr).await else {
                    network_wire.push_back(Frame::Eos);
                    break;
                };
                buffer.put(chunk.content.as_slice());

                let mut proof = if block == 0 {
                    ProofBuf::new(&tree.tree, 0)
                } else {
                    ProofBuf::resume(&tree.tree, block)
                };

                while !buffer.is_empty() {
                    if !proof.is_empty() {
                        network_wire.push_back(Frame::Proof(proof.as_slice().to_vec()));
                    };

                    let bytes = buffer.split_to(buffer.len().min(BLOCK_SIZE));
                    network_wire.push_back(Frame::Chunk(bytes.to_vec()));
                    block += 1;
                    if block < num_blocks {
                        proof = ProofBuf::resume(&tree.tree, block)
                    }
                }
            }
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
            .pubkey_to_index(peers[0].node_public_key)
            .unwrap();

        let content = create_content();
        // Put some data into the blockstore of peer 1
        let mut putter = peers[0].blockstore.put(None);
        putter
            .write(&content, CompressionAlgorithm::Uncompressed)
            .unwrap();
        let hash = putter.finalize().await.unwrap();

        // Send a request from peer 2 to peer 1
        let (tx, rx) = oneshot::channel();
        let server_req = ServerRequest {
            hash,
            peer: node_index1,
            response: tx,
        };
        peers[1].request_tx.send(server_req).await.unwrap();
        let mut res = rx.await.unwrap();
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
}
