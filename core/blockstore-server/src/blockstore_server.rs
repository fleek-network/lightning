//! TODO: Once affair supports a `UnorderedWorker` trait, we should use that
//!       to simplify and correct the `Socket` usage here to avoid `raw_bounded`.

use std::borrow::Cow;
use std::collections::{HashMap, VecDeque};
use std::mem;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use affair::{Socket, Task};
use anyhow::{anyhow, Result};
use blake3_tree::ProofBuf;
use bytes::{Buf, BufMut, Bytes, BytesMut};
use lightning_interfaces::prelude::*;
use lightning_interfaces::types::{
    Blake3Hash,
    CompressionAlgoSet,
    CompressionAlgorithm,
    NodeIndex,
    PeerRequestError,
    RejectReason,
    ServerRequest,
};
use lightning_interfaces::ServiceScope;
use lightning_metrics::increment_counter;
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, mpsc};
use tokio::task::JoinSet;
use tokio::time::timeout;
use tokio_stream::StreamExt;
use tracing::error;

use crate::config::Config;

type ServerRequestTask = Task<ServerRequest, broadcast::Receiver<Result<(), PeerRequestError>>>;

const REQUEST_TIMEOUT: Duration = Duration::from_millis(1000);

pub struct BlockstoreServer<C: Collection> {
    inner: Option<BlockstoreServerInner<C>>,
    socket: BlockstoreServerSocket,
}

impl<C: Collection> BlockstoreServerInterface<C> for BlockstoreServer<C> {
    fn get_socket(&self) -> BlockstoreServerSocket {
        self.socket.clone()
    }
}

impl<C: Collection> BlockstoreServer<C> {
    fn init(
        config: &C::ConfigProviderInterface,
        blockstore: &C::BlockstoreInterface,
        pool: &C::PoolInterface,
        rep_aggregator: &C::ReputationAggregatorInterface,
    ) -> Result<Self> {
        let config = config.get::<Self>();
        let (pool_requester, pool_responder) = pool.open_req_res(ServiceScope::BlockstoreServer);
        let (socket, request_rx) = Socket::raw_bounded(2048);
        let inner = Some(BlockstoreServerInner::<C>::new(
            blockstore.clone(),
            request_rx,
            config.max_conc_req,
            config.max_conc_res,
            pool_requester,
            pool_responder,
            rep_aggregator.get_reporter(),
        ));

        Ok(Self { inner, socket })
    }

    /// Start the system, should only be called once
    async fn start(mut this: fdi::RefMut<Self>, waiter: fdi::Cloned<ShutdownWaiter>) {
        let inner = this
            .inner
            .take()
            .expect("start should never be called twice");
        drop(this);
        waiter.run_until_shutdown(inner.start()).await;
    }
}

impl<C: Collection> BuildGraph for BlockstoreServer<C> {
    fn build_graph() -> fdi::DependencyGraph {
        fdi::DependencyGraph::default().with(Self::init.on("start", Self::start.spawn()))
    }
}

#[allow(clippy::type_complexity)]
pub struct BlockstoreServerInner<C: Collection> {
    blockstore: C::BlockstoreInterface,
    request_rx: mpsc::Receiver<ServerRequestTask>,
    max_conc_req: usize,
    max_conc_res: usize,
    num_responses: Arc<AtomicUsize>,
    pool_requester: c!(C::PoolInterface::Requester),
    pool_responder: c!(C::PoolInterface::Responder),
    rep_reporter: c!(C::ReputationAggregatorInterface::ReputationReporter),
}

impl<C: Collection> BlockstoreServerInner<C> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        blockstore: C::BlockstoreInterface,
        request_rx: mpsc::Receiver<ServerRequestTask>,
        max_conc_req: usize,
        max_conc_res: usize,
        pool_requester: c!(C::PoolInterface::Requester),
        pool_responder: c!(C::PoolInterface::Responder),
        rep_reporter: c!(C::ReputationAggregatorInterface::ReputationReporter),
    ) -> Self {
        Self {
            blockstore,
            request_rx,
            max_conc_req,
            max_conc_res,
            num_responses: AtomicUsize::new(0).into(),
            pool_requester,
            pool_responder,
            rep_reporter,
        }
    }

    pub async fn start(mut self) {
        let mut pending_requests: HashMap<
            PeerRequest,
            broadcast::Sender<Result<(), PeerRequestError>>,
        > = HashMap::new();
        let mut tasks = JoinSet::new();
        let mut queue = VecDeque::new();

        // Hack to force the JoinSet to never return `None`. This simplifies the tokio::select.
        tasks.spawn(futures::future::pending());

        loop {
            tokio::select! {
                req = self.pool_responder.get_next_request() => {
                    match req {
                        Ok((req_header, responder)) => {
                            // TODO(matthias): find out which peer the request came from
                            match PeerRequest::try_from(req_header.bytes) {
                                Ok(request) => {
                                    let num_res = self.num_responses.fetch_add(1, Ordering::AcqRel);
                                    if num_res < self.max_conc_res {
                                        let blockstore = self.blockstore.clone();
                                        let num_responses = self.num_responses.clone();
                                        let rep_reporter = self.rep_reporter.clone();
                                        tokio::spawn(async move {
                                            handle_request::<C>(
                                                req_header.peer,
                                                request,
                                                blockstore,
                                                responder,
                                                num_responses,
                                                rep_reporter,
                                            ).await;

                                            increment_counter!(
                                                "blockstore_server_handle_request",
                                                Some("Counter for number of blockstore requests handled by this node")
                                            );
                                        });
                                    } else {
                                        self.num_responses.fetch_sub(1, Ordering::Release);
                                        responder.reject(RejectReason::TooManyRequests);
                                    }
                                }
                                Err(e) => error!("Failed to decode request from peer: {e:?}"),
                            }
                        }
                        Err(e) => {
                            error!("Failed to receive request from pool: {e:?}");
                            break;
                        }
                    }
                }
                task = self.request_rx.recv() => {
                    if let Some(task) = task {
                        let peer_request = PeerRequest { hash: task.request.hash };
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
                                let rep_reporter = self.rep_reporter.clone();
                                tasks.spawn(async move {
                                    let res = send_request::<C>(
                                        task.request.peer,
                                        peer_request_,
                                        blockstore,
                                        pool_requester,
                                        rep_reporter,
                                    ).await;

                                    if res.is_ok() {
                                        increment_counter!(
                                           "blockstore_server_send_request_ok",
                                           Some("Counter for the number of successful blockstore requests made")
                                        );
                                    } else {
                                        increment_counter!(
                                            "blockstore_servier_send_request_err",
                                            Some("Counter for the number of failed blockstore requests made")
                                        );
                                    }

                                    res
                                });
                            } else {
                                queue.push_back(peer_request.clone());
                            }
                            let (tx, rx) = broadcast::channel(1);
                            pending_requests.insert(peer_request, tx);
                            rx
                        };
                        task.respond(rx);
                    } else {
                        break;
                    }
                }
                Some(res) = tasks.join_next() => {
                    match res {
                        Ok(Ok(peer_request)) => {
                            if let Some(tx) = pending_requests.remove(&peer_request) {
                                tx.send(Ok(())).expect("Failed to send response");
                            }
                        },
                        Ok(Err(error_res)) => {
                            error!("Failed to fetch data from peer: {:?}", error_res.error);
                            if let Some(tx) = pending_requests.remove(&error_res.request) {
                                tx.send(Err(error_res.error)).expect("Failed to send response");
                            }
                        },
                        Err(e) => error!("Failed to join task: {e:?}"),
                    }
                }
            }
        }
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

pub enum Frame<'a> {
    Proof(Cow<'a, [u8]>),
    Chunk(Cow<'a, [u8]>),
    Eos,
}

impl<'a> From<Frame<'a>> for Bytes {
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

impl TryFrom<Bytes> for Frame<'static> {
    type Error = anyhow::Error;

    fn try_from(mut value: Bytes) -> Result<Self> {
        match value.get_u8() {
            0x00 => Ok(Frame::Proof(Cow::Owned(value.to_vec()))),
            0x01 => Ok(Frame::Chunk(Cow::Owned(value.to_vec()))),
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
    peer: NodeIndex,
    peer_request: PeerRequest,
    blockstore: C::BlockstoreInterface,
    mut request: <c!(C::PoolInterface::Responder) as ResponderInterface>::Request,
    num_responses: Arc<AtomicUsize>,
    rep_reporter: c!(C::ReputationAggregatorInterface::ReputationReporter),
) {
    if let Some(tree) = blockstore.get_tree(&peer_request.hash).await {
        let mut num_bytes = 0;
        let instant = Instant::now();
        for block in 0..tree.len() {
            let compr = CompressionAlgoSet::default(); // rustfmt
            let Some(chunk) = blockstore.get(block as u32, &tree[block], compr).await else {
                break;
            };

            let proof = if block == 0 {
                ProofBuf::new(tree.as_ref(), 0)
            } else {
                ProofBuf::resume(tree.as_ref(), block)
            };

            if !proof.is_empty() {
                num_bytes += proof.len();
                if let Err(e) = request
                    .send(Bytes::from(Frame::Proof(Cow::Borrowed(proof.as_slice()))))
                    .await
                {
                    error!("Failed to send proof: {e:?}");
                    num_responses.fetch_sub(1, Ordering::Release);
                    return;
                }
            }

            num_bytes += chunk.content.len();
            if let Err(e) = request
                .send(Bytes::from(Frame::Chunk(Cow::Borrowed(
                    chunk.content.as_slice(),
                ))))
                .await
            {
                error!("Failed to send chunk: {e:?}");
                num_responses.fetch_sub(1, Ordering::Release);
                return;
            }
        }
        if let Err(e) = request.send(Bytes::from(Frame::Eos)).await {
            error!("Failed to send eos: {e:?}");
        } else {
            rep_reporter.report_bytes_sent(peer, num_bytes as u64, Some(instant.elapsed()));
        }
    } else {
        request.reject(RejectReason::ContentNotFound);
    }

    num_responses.fetch_sub(1, Ordering::Release);
}

async fn send_request<C: Collection>(
    peer: NodeIndex,
    request: PeerRequest,
    blockstore: C::BlockstoreInterface,
    pool_requester: c!(C::PoolInterface::Requester),
    rep_reporter: c!(C::ReputationAggregatorInterface::ReputationReporter),
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
                    let mut bytes_recv = 0;
                    let instant = Instant::now();

                    while let Some(bytes) = body.next().await {
                        let Ok(bytes) = bytes else {
                            return Err(ErrorResponse {
                                error: PeerRequestError::Incomplete,
                                request,
                            });
                        };
                        bytes_recv += bytes.len();
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
                                // TODO: Handle premature end of stream errors instead of
                                // unwrapping here, since we there could be an upstream blockstore
                                // miss where the server would send an EOS frame.
                                let _hash = putter.finalize().await.unwrap();
                                // TODO(matthias): do we have to compare this hash to the
                                // requested hash?
                                let duration = instant.elapsed();
                                rep_reporter.report_bytes_received(
                                    peer,
                                    bytes_recv as u64,
                                    Some(duration),
                                );
                                return Ok(request);
                            },
                        }
                    }
                    Err(ErrorResponse {
                        error: PeerRequestError::Incomplete,
                        request,
                    })
                },
                Err(reason) => Err(ErrorResponse {
                    error: PeerRequestError::Rejected(reason),
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

impl<C: Collection> ConfigConsumer for BlockstoreServer<C> {
    const KEY: &'static str = "blockstore-server";

    type Config = Config;
}
