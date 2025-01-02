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
use b3fs::bucket::dir::reader::B3Dir;
use b3fs::entry::{BorrowedEntry, InlineVec, OwnedEntry, OwnedLink};
use bytes::{Buf, BufMut, Bytes, BytesMut};
use lightning_interfaces::prelude::*;
use lightning_interfaces::types::{
    Blake3Hash,
    NodeIndex,
    PeerRequestError,
    RejectReason,
    ServerRequest,
};
use lightning_interfaces::{
    DirTrustedWriter,
    DirUntrustedWriter,
    FileTrustedWriter,
    FileUntrustedWriter,
    ServiceScope,
};
use lightning_metrics::increment_counter;
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, mpsc, RwLock};
use tokio::task::JoinSet;
use tokio::time::timeout;
use tokio_stream::StreamExt;
use tracing::error;

use crate::config::Config;

type ServerRequestTask = Task<ServerRequest, broadcast::Receiver<Result<(), PeerRequestError>>>;

const REQUEST_TIMEOUT: Duration = Duration::from_millis(1000);

pub struct BlockstoreServer<C: NodeComponents> {
    inner: Option<BlockstoreServerInner<C>>,
    socket: BlockstoreServerSocket,
}

impl<C: NodeComponents> BlockstoreServerInterface<C> for BlockstoreServer<C> {
    fn get_socket(&self) -> BlockstoreServerSocket {
        self.socket.clone()
    }
}

impl<C: NodeComponents> BlockstoreServer<C> {
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

impl<C: NodeComponents> BuildGraph for BlockstoreServer<C> {
    fn build_graph() -> fdi::DependencyGraph {
        fdi::DependencyGraph::default().with(Self::init.with_event_handler(
            "start",
            Self::start.wrap_with_spawn_named("BLOCKSTORE-SERVER"),
        ))
    }
}

#[allow(clippy::type_complexity)]
pub struct BlockstoreServerInner<C: NodeComponents> {
    blockstore: C::BlockstoreInterface,
    request_rx: mpsc::Receiver<ServerRequestTask>,
    max_conc_req: usize,
    max_conc_res: usize,
    num_responses: Arc<AtomicUsize>,
    pool_requester: c!(C::PoolInterface::Requester),
    pool_responder: c!(C::PoolInterface::Responder),
    rep_reporter: c!(C::ReputationAggregatorInterface::ReputationReporter),
}

impl<C: NodeComponents> BlockstoreServerInner<C> {
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
                                        spawn!(
                                            async move {
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
                                            },
                                            "BLOCKSTORE-SERVER: handle request"
                                        );
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
}

impl From<PeerRequest> for Bytes {
    fn from(value: PeerRequest) -> Self {
        let mut buf = BytesMut::with_capacity(value.hash.len());
        buf.put_slice(&value.hash);
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
        Ok(Self {
            hash: hash.to_vec().try_into().unwrap(),
        })
    }
}

#[derive(Debug)]
pub enum Frame<'a> {
    File(FileFrame<'a>),
    Dir(DirFrame<'a>),
}

#[derive(Debug)]
pub enum FileFrame<'a> {
    Prelude([u8; 32]),
    Proof(Cow<'a, [u8]>),
    Chunk(Cow<'a, [u8]>),
    LastChunk(Cow<'a, [u8]>),
    Eos,
}

#[derive(Debug)]
pub enum DirFrame<'a> {
    Prelude((u32, [u8; 32])),
    Proof(Cow<'a, [u8]>),
    Chunk(Cow<'a, OwnedEntry>),
    LastChunk(Cow<'a, OwnedEntry>),
    Eos,
}

impl<'a> DirFrame<'a> {
    fn entry_len(entry: &OwnedEntry) -> usize {
        let mut bytes = entry.name.len();
        match &entry.link {
            OwnedLink::Content(c) => bytes += c.len(),
            OwnedLink::Link(l) => bytes += l.len(),
        }
        bytes
    }

    pub fn len(&self) -> usize {
        match &self {
            Self::Prelude((entries, hash)) => hash.len() + entries.to_le_bytes().len(),
            Self::Proof(c) => c.len(),
            Self::Chunk(entry) => Self::entry_len(entry),
            Self::LastChunk(entry) => Self::entry_len(entry),
            Self::Eos => 0,
        }
    }

    pub fn from_entry(value: OwnedEntry, last: bool) -> Self {
        if last {
            Self::LastChunk(Cow::Owned(value))
        } else {
            Self::Chunk(Cow::Owned(value))
        }
    }
}

impl<'a> From<Frame<'a>> for Bytes {
    fn from(value: Frame) -> Self {
        let mut b = BytesMut::new();
        match value {
            Frame::File(file) => match file {
                FileFrame::Prelude(hash) => {
                    b.put_u8(0x00);
                    b.put_slice(&hash);
                },
                FileFrame::Proof(proof) => {
                    b.put_u8(0x01);
                    b.put_slice(&proof);
                },
                FileFrame::Chunk(chunk) => {
                    b.put_u8(0x02);
                    b.put_slice(&chunk);
                },
                FileFrame::LastChunk(chunk) => {
                    b.put_u8(0x03);
                    b.put_slice(&chunk);
                },
                FileFrame::Eos => {
                    b.put_u8(0x04);
                },
            },
            Frame::Dir(dir) => match dir {
                DirFrame::Prelude((num_entries, hash)) => {
                    b.put_u8(0x10);
                    b.put_u32_le(num_entries);
                    b.put_slice(&hash);
                },
                DirFrame::Proof(proof) => {
                    b.put_u8(0x11);
                    b.put_slice(&proof);
                },
                DirFrame::Chunk(chunk) => {
                    b.put_u8(0x12);
                    b.put_slice(&chunk.name);
                    b.put_u8(0x00);
                    match &chunk.link {
                        b3fs::entry::OwnedLink::Content(content) => {
                            b.put_u8(0x01);
                            b.put_slice(content);
                        },
                        b3fs::entry::OwnedLink::Link(link) => {
                            b.put_u8(0x02);
                            b.put_slice(link);
                        },
                    }
                },
                DirFrame::LastChunk(chunk) => {
                    b.put_u8(0x13);
                    b.put_slice(&chunk.name);
                    b.put_u8(0x00);
                    match &chunk.link {
                        b3fs::entry::OwnedLink::Content(content) => {
                            b.put_u8(0x01);
                            b.put_slice(content);
                        },
                        b3fs::entry::OwnedLink::Link(link) => {
                            b.put_u8(0x02);
                            b.put_slice(link);
                        },
                    }
                },
                DirFrame::Eos => {
                    b.put_u8(0x14);
                },
            },
        }
        b.freeze()
    }
}

impl TryFrom<Bytes> for Frame<'static> {
    type Error = anyhow::Error;

    fn try_from(mut value: Bytes) -> Result<Self> {
        let val_frame = value.get_u8();
        match val_frame {
            0x00 => {
                let mut content: [u8; 32] = [0; 32];
                content.copy_from_slice(&value);
                Ok(Frame::File(FileFrame::Prelude(content)))
            },
            0x01 => Ok(Frame::File(FileFrame::Proof(Cow::Owned(value.to_vec())))),
            0x02 => Ok(Frame::File(FileFrame::Chunk(Cow::Owned(value.to_vec())))),
            0x03 => Ok(Frame::File(FileFrame::LastChunk(Cow::Owned(
                value.to_vec(),
            )))),
            0x04 => Ok(Frame::File(FileFrame::Eos)),
            0x10 => {
                let num_entries = value.get_u32_le();
                let mut content: [u8; 32] = [0; 32];
                content.copy_from_slice(&value);
                Ok(Frame::Dir(DirFrame::Prelude((num_entries, content))))
            },
            0x11 => Ok(Frame::Dir(DirFrame::Proof(Cow::Owned(value.to_vec())))),
            0x12 => {
                let bytes: &[u8] = if let Some(bs) = value.iter().position(|p| *p == 0x00) {
                    &value.split_to(bs)
                } else {
                    return Err(anyhow!("Error detecting null byte for name"));
                };
                if bytes.len() > 24 {
                    return Err(anyhow!(
                        "We are receiving more bytes than allowed for InlineVec"
                    ));
                }
                let _ = value.get_u8();
                let name: InlineVec = bytes.into();
                let ty = value.get_u8();
                match ty {
                    0x01 => {
                        let mut content = [0; 32];
                        let bytes = value.copy_to_bytes(32);
                        content.copy_from_slice(&bytes);
                        Ok(Frame::Dir(DirFrame::Chunk(Cow::Owned(OwnedEntry {
                            name,
                            link: b3fs::entry::OwnedLink::Content(content),
                        }))))
                    },
                    0x02 => {
                        let mut link = [0; 24];
                        if value.len() > 24 {
                            return Err(anyhow!(
                                "We are receiving more bytes than allowed for InlineVec"
                            ));
                        }
                        link.copy_from_slice(&value);
                        let link = InlineVec::from_buf(link);
                        Ok(Frame::Dir(DirFrame::Chunk(Cow::Owned(OwnedEntry {
                            name,
                            link: b3fs::entry::OwnedLink::Link(link),
                        }))))
                    },
                    _ => Err(anyhow!("Unknown magic byte for OwnedEntry type")),
                }
            },
            0x13 => {
                let bytes: &[u8] = if let Some(bs) = value.iter().position(|p| *p == 0x00) {
                    &value.split_to(bs)
                } else {
                    return Err(anyhow!("Error detecting null byte for name"));
                };
                if bytes.len() > 24 {
                    return Err(anyhow!(
                        "We are receiving more bytes than allowed for InlineVec"
                    ));
                }
                let _ = value.get_u8();
                let name: InlineVec = bytes.into();
                let ty = value.get_u8();
                match ty {
                    0x01 => {
                        let mut content = [0; 32];
                        let bytes = value.copy_to_bytes(32);
                        content.copy_from_slice(&bytes);
                        Ok(Frame::Dir(DirFrame::LastChunk(Cow::Owned(OwnedEntry {
                            name,
                            link: b3fs::entry::OwnedLink::Content(content),
                        }))))
                    },
                    0x02 => {
                        let mut link = [0; 24];
                        if value.len() > 24 {
                            return Err(anyhow!(
                                "We are receiving more bytes than allowed for InlineVec"
                            ));
                        }

                        link.copy_from_slice(&value);
                        let link = InlineVec::from_buf(link);
                        Ok(Frame::Dir(DirFrame::LastChunk(Cow::Owned(OwnedEntry {
                            name,
                            link: b3fs::entry::OwnedLink::Link(link),
                        }))))
                    },
                    _ => Err(anyhow!("Unknown magic byte for OwnedEntry type")),
                }
            },
            0x14 => Ok(Frame::Dir(DirFrame::Eos)),
            i => Err(anyhow!("Unknown magic byte {i}")),
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

async fn handle_request<C: NodeComponents>(
    peer: NodeIndex,
    peer_request: PeerRequest,
    blockstore: C::BlockstoreInterface,
    mut request: <c!(C::PoolInterface::Responder) as ResponderInterface>::Request,
    num_responses: Arc<AtomicUsize>,
    rep_reporter: c!(C::ReputationAggregatorInterface::ReputationReporter),
) {
    if let Ok(tree) = blockstore.get_bucket().get(&peer_request.hash).await {
        let num_blocks = tree.blocks();
        if tree.is_file() {
            if let Some(file) = tree.into_file() {
                match send_file::<C>(
                    file,
                    peer_request.hash,
                    &mut request,
                    num_blocks,
                    &num_responses,
                    blockstore,
                    rep_reporter,
                    peer,
                )
                .await
                {
                    Ok(_) => (),
                    Err(e) => {
                        error!("{e:?}");
                        return request.reject(RejectReason::ContentNotFound);
                    },
                }
            } else {
                error!(
                    "Content Header detected a File type but it could not be converted into File"
                );
                request.reject(RejectReason::Other);
            }
        } else if let Some(dir) = tree.into_dir() {
            match send_dir::<C>(
                dir,
                peer_request.hash,
                &mut request,
                blockstore,
                num_blocks,
                &num_responses,
                rep_reporter,
                peer,
            )
            .await
            {
                Ok(_) => (),
                Err(e) => {
                    error!("{e:?}");
                    return request.reject(RejectReason::ContentNotFound);
                },
            }
        } else {
            error!(
                "Content Header detected a Directory type but it could not be converted into Dir"
            );
            request.reject(RejectReason::Other);
        }
    } else {
        request.reject(RejectReason::ContentNotFound);
    }

    num_responses.fetch_sub(1, Ordering::Release);
}

async fn send_file<C: NodeComponents>(
    file: b3fs::bucket::file::reader::B3File,
    hash: [u8; 32],
    request: &mut <c!(C::PoolInterface::Responder) as ResponderInterface>::Request,
    num_blocks: u32,
    num_responses: &AtomicUsize,
    blockstore: <C as NodeComponents>::BlockstoreInterface,
    rep_reporter: c!(C::ReputationAggregatorInterface::ReputationReporter),
    peer: u32,
) -> anyhow::Result<()> {
    let mut num_bytes = 0;
    let instant = Instant::now();
    let mut reader = match file.hashtree().await {
        Ok(reader) => reader,
        Err(e) => {
            return Err(anyhow!("Failed to get Async HashTree {}", e));
        },
    };
    if let Err(e) = request
        .send(Bytes::from(Frame::File(FileFrame::Prelude(hash))))
        .await
    {
        num_responses.fetch_sub(1, Ordering::Release);
        return Err(anyhow!("Failed to send prelude file: {e:?}"));
    }
    for block in 0..num_blocks {
        let hash = match reader.get_hash(block).await {
            Ok(Some(hash)) => hash,
            Ok(_) => break,
            Err(e) => {
                return Err(anyhow!("Failed to read hash from block {} - {}", block, e));
            },
        };

        let proof = match reader.generate_proof(block).await {
            Ok(p) => p,
            Err(e) => {
                return Err(anyhow!("Failed to generate proof {}", e));
            },
        };

        num_bytes += proof.len();
        if let Err(e) = request
            .send(Bytes::from(Frame::File(FileFrame::Proof(Cow::Borrowed(
                proof.as_slice(),
            )))))
            .await
        {
            num_responses.fetch_sub(1, Ordering::Release);
            return Err(anyhow!("Failed to send proof: {e:?}"));
        }

        let chunk = match blockstore.get_bucket().get_block_content(&hash).await {
            Ok(Some(chunk)) => chunk,
            Ok(None) => return Err(anyhow!("Cannot get block content")),
            Err(e) => return Err(anyhow!("Cannot get block content {e:?}")),
        };

        num_bytes += chunk.len();
        let frame = if block == num_blocks - 1 {
            Frame::File(FileFrame::LastChunk(Cow::Borrowed(&chunk)))
        } else {
            Frame::File(FileFrame::Chunk(Cow::Borrowed(&chunk)))
        };
        if let Err(e) = request.send(Bytes::from(frame)).await {
            num_responses.fetch_sub(1, Ordering::Release);
            return Err(anyhow!("Failed to send chunk: {e:?}"));
        }
    }
    if let Err(e) = request.send(Bytes::from(Frame::File(FileFrame::Eos))).await {
        return Err(anyhow!("Failed to send eos: {e:?}"));
    } else {
        rep_reporter.report_bytes_sent(peer, num_bytes as u64, Some(instant.elapsed()));
    }
    Ok(())
}

struct QueueDir {
    dir: B3Dir,
    num_blocks: u32,
    hash: [u8; 32],
    prelude_sent: bool,
}

impl QueueDir {
    pub fn new(dir: B3Dir, hash: [u8; 32], num_blocks: u32) -> Self {
        QueueDir {
            dir,
            num_blocks,
            hash,
            prelude_sent: false,
        }
    }
}

async fn send_dir<C: NodeComponents>(
    dir: b3fs::bucket::dir::reader::B3Dir,
    hash: [u8; 32],
    request: &mut <c!(C::PoolInterface::Responder) as ResponderInterface>::Request,
    blockstore: <C as NodeComponents>::BlockstoreInterface,
    num_blocks: u32,
    num_responses: &AtomicUsize,
    rep_reporter: c!(C::ReputationAggregatorInterface::ReputationReporter),
    peer: u32,
) -> anyhow::Result<()> {
    let mut num_bytes = 0;
    let instant = Instant::now();

    let mut stack_reader = VecDeque::new();
    stack_reader.push_back(QueueDir::new(dir, hash, num_blocks));

    'dir_reader: loop {
        if let Some(ref mut queue_dir) = stack_reader.pop_front() {
            dbg!("New dir");
            let mut reader = match queue_dir.dir.hashtree().await {
                Ok(reader) => reader,
                Err(e) => {
                    return Err(anyhow!("Failed to get Async HashTree {}", e));
                },
            };
            let mut entries_reader = match queue_dir.dir.entries().await {
                Ok(entries) => entries,
                Err(e) => {
                    return Err(anyhow!("Error trying to obtain Entries Iterator {}", e));
                },
            };
            if !queue_dir.prelude_sent {
                if let Err(e) = request
                    .send(Bytes::from(Frame::Dir(DirFrame::Prelude((
                        queue_dir.num_blocks,
                        queue_dir.hash,
                    )))))
                    .await
                {
                    return Err(anyhow!("Failed to send prelude: {e:?}"));
                } else {
                    rep_reporter.report_bytes_sent(peer, num_bytes as u64, Some(instant.elapsed()));
                    queue_dir.prelude_sent = true;
                }
            }

            let mut block = 0;
            'entries: while let Some(ent) = entries_reader.next().await {
                dbg!("new entry");
                match ent {
                    Ok(ref entry) => {
                        let proof = match reader.generate_proof(block).await {
                            Ok(p) => p,
                            Err(e) => {
                                return Err(anyhow!("Failed to generate proof {}", e));
                            },
                        };
                        num_bytes += proof.len();
                        if let Err(e) = request
                            .send(Bytes::from(Frame::Dir(DirFrame::Proof(Cow::Borrowed(
                                proof.as_slice(),
                            )))))
                            .await
                        {
                            num_responses.fetch_sub(1, Ordering::Release);
                            return Err(anyhow!("Failed to send proof: {e:?}"));
                        }
                        let dir_frame: DirFrame<'_> =
                            DirFrame::from_entry(entry.clone(), num_blocks == block + 1);
                        num_bytes += dir_frame.len();
                        let frame = Frame::Dir(dir_frame);

                        if let Err(e) = request.send(Bytes::from(frame)).await {
                            error!("Failed to send chunk: {e:?}");
                            num_responses.fetch_sub(1, Ordering::Release);
                        }
                        block += 1;
                        match entry.link {
                            OwnedLink::Content(content) => {
                                let content_header = blockstore.get_bucket().get(&content).await;
                                match content_header {
                                    Ok(cont_head) => {
                                        let num_blocks_new_content = cont_head.blocks();
                                        if cont_head.is_dir() {
                                            if let Some(dir_reader) = cont_head.into_dir() {
                                                stack_reader.push_back(QueueDir::new(
                                                    dir_reader,
                                                    content,
                                                    num_blocks_new_content,
                                                ));
                                                continue 'entries;
                                            } else {
                                                return Err(anyhow!(
                                                    "Content was detect as DIR but it cannot be converted into DirReader"
                                                ));
                                            }
                                        } else if let Some(file_data) = cont_head.into_file() {
                                            dbg!("sending file");
                                            send_file::<C>(
                                                file_data,
                                                content,
                                                request,
                                                num_blocks_new_content,
                                                num_responses,
                                                blockstore.clone(),
                                                rep_reporter.clone(),
                                                peer,
                                            )
                                            .await?;
                                            continue 'entries;
                                        } else {
                                            return Err(anyhow!(
                                                "File Reader cannot be obtained and it should be present"
                                            ));
                                        }
                                    },
                                    Err(err) => {
                                        return Err(anyhow!("Error getting entry - {}", err));
                                    },
                                };
                            },
                            OwnedLink::Link(_) => (),
                        };
                    },
                    Err(e) => {
                        return Err(anyhow!("Error getting entry - {}", e));
                    },
                }
            }
            if let Err(e) = request.send(Bytes::from(Frame::Dir(DirFrame::Eos))).await {
                return Err(anyhow!("Failed to send eos: {e:?}"));
            } else {
                rep_reporter.report_bytes_sent(peer, num_bytes as u64, Some(instant.elapsed()));
            }
        } else {
            break 'dir_reader;
        }
    }
    Ok(())
}

async fn send_request<C: NodeComponents>(
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
        Ok(Ok(response)) => match response.status_code() {
            Ok(()) => {
                let mut body = response.body();
                let mut writer: Option<
                    RwLock<<C::BlockstoreInterface as BlockstoreInterface<C>>::UFileWriter>,
                > = None;
                let mut dir_writer: Option<
                    RwLock<<C::BlockstoreInterface as BlockstoreInterface<C>>::UDirWriter>,
                > = None;

                let mut bytes_recv = 0;
                let instant = Instant::now();

                while let Some(bytes) = body.next().await {
                    let bytes = match bytes {
                        Ok(b) => b,
                        Err(e) => {
                            error!("{}", e);
                            return Err(ErrorResponse {
                                error: PeerRequestError::Incomplete,
                                request,
                            });
                        },
                    };
                    bytes_recv += bytes.len();
                    let Ok(frame) = Frame::try_from(bytes) else {
                        return Err(ErrorResponse {
                            error: PeerRequestError::Incomplete,
                            request,
                        });
                    };
                    match frame {
                        Frame::File(file) => {
                            if let FileFrame::Prelude(hash) = file {
                                writer = Some(RwLock::new(
                                    blockstore.file_untrusted_writer(hash).await.unwrap(),
                                ));
                            };
                            if let Some(ref file_writer) = writer {
                                match handle_send_request_file::<C>(file_writer, file).await {
                                    Ok(RespSendRequest::Continue) => (),
                                    Ok(RespSendRequest::EoF) => {
                                        let writer = writer.take().unwrap().into_inner();
                                        writer.commit().await.expect("Error commiting writer");
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
                                    Err(err) => {
                                        error!("Error handling send request for file {}", err);
                                        break;
                                    },
                                }
                            } else {
                                error!("File Writer could not be initialized");
                                break;
                            }
                        },
                        Frame::Dir(dir) => {
                            if let DirFrame::Prelude((num_entries, hash)) = dir {
                                dir_writer = Some(RwLock::new(
                                    blockstore
                                        .dir_untrusted_writer(hash, num_entries as usize)
                                        .await
                                        .unwrap(),
                                ));
                            };
                            if let Some(ref dir_wr) = dir_writer {
                                match handle_send_request_dir::<C>(dir_wr, dir).await {
                                    Ok(RespSendRequest::Continue) => (),
                                    Ok(RespSendRequest::EoF) => {
                                        let dir_writer = dir_writer.take().unwrap().into_inner();
                                        dir_writer.commit().await.expect("Error commiting writer");
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
                                    Err(err) => {
                                        error!("Error handling send request for dir {}", err);
                                        break;
                                    },
                                }
                            } else {
                                error!("Dir Writer was not initilialized properly");
                                break;
                            }
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

enum RespSendRequest {
    Continue,
    EoF,
}

async fn handle_send_request_file<C: NodeComponents>(
    writer: &RwLock<<C::BlockstoreInterface as BlockstoreInterface<C>>::UFileWriter>,
    file: FileFrame<'_>,
) -> Result<RespSendRequest, String> {
    match file {
        FileFrame::Prelude(_) => Ok(RespSendRequest::Continue),
        FileFrame::Proof(proof) => writer
            .write()
            .await
            .feed_proof(&proof)
            .await
            .map(|_| RespSendRequest::Continue)
            .map_err(|e| e.to_string()),
        FileFrame::Chunk(chunk) => writer
            .write()
            .await
            .write(&chunk, false)
            .await
            .map(|_| RespSendRequest::Continue)
            .map_err(|e| e.to_string()),
        FileFrame::LastChunk(chunk) => writer
            .write()
            .await
            .write(&chunk, true)
            .await
            .map(|_| RespSendRequest::Continue)
            .map_err(|e| e.to_string()),
        FileFrame::Eos => Ok(RespSendRequest::EoF),
    }
}

async fn handle_send_request_dir<C: NodeComponents>(
    writer: &RwLock<<C::BlockstoreInterface as BlockstoreInterface<C>>::UDirWriter>,
    dir: DirFrame<'_>,
) -> Result<RespSendRequest, String> {
    match dir {
        DirFrame::Prelude(_) => Ok(RespSendRequest::Continue),
        DirFrame::Proof(proof) => writer
            .write()
            .await
            .feed_proof(&proof)
            .await
            .map(|_| RespSendRequest::Continue)
            .map_err(|e| e.to_string()),
        DirFrame::Chunk(chunk) => writer
            .write()
            .await
            .insert(BorrowedEntry::from(&chunk.into_owned()), false)
            .await
            .map(|_| RespSendRequest::Continue)
            .map_err(|e| e.to_string()),
        DirFrame::LastChunk(chunk) => writer
            .write()
            .await
            .insert(BorrowedEntry::from(&chunk.into_owned()), true)
            .await
            .map(|_| RespSendRequest::Continue)
            .map_err(|e| e.to_string()),
        DirFrame::Eos => Ok(RespSendRequest::EoF),
    }
}

impl<C: NodeComponents> ConfigConsumer for BlockstoreServer<C> {
    const KEY: &'static str = "blockstore-server";

    type Config = Config;
}
