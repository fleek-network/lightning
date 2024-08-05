use std::cell::RefCell;
use std::collections::{BinaryHeap, HashMap, VecDeque};

use futures::executor::LocalPool;
use fxhash::FxHashMap;
use triomphe::Arc;

use crate::api::{ConnectError, RemoteAddr};
use crate::future::{DeferredFuture, DeferredFutureWaker};
use crate::message::{Ignored, Message, MessageDetail};
use crate::report::{Metrics, NodeMetrics};
use crate::storage::TypedStorage;

thread_local! {
    static NODES: RefCell<*mut NodeState> = const { RefCell::new(std::ptr::null_mut()) };
}

/// Provide the node that is being executed on the current thread.
pub fn hook_node(node: *mut NodeState) {
    NODES.with(|cell| {
        cell.replace(node);
    })
}

/// Provide the [`NodeState`] of the node that is currently being executed to the passed
/// closure.
pub fn with_node<F, T>(f: F) -> T
where
    F: FnOnce(&mut NodeState) -> T,
{
    NODES.with(|cell| {
        let ptr = *cell.borrow();

        if ptr.is_null() {
            panic!("Simulon API function used outside of executor.");
        }

        let mut_ref = unsafe { &mut *ptr };

        f(mut_ref)
    })
}

/// The state of a single node.
pub struct NodeState {
    /// The global index of this node.
    pub node_id: usize,
    /// The total number of nodes in the simulation.
    pub count_nodes: usize,
    /// The current epoch time in the simulation.
    pub time: u128,
    /// The pool of futures that we need to execute.
    pub spawn_pool: LocalPool,
    /// The resource table that holds the mapping from a resource id to the resource.
    pub resources: HashMap<ResourceId, Resource>,
    /// Ports that we're listening on.
    pub listening: FxHashMap<u16, ListenerState>,
    /// The array of pending outgoing requests.
    pub outgoing: Vec<Message>,
    /// The messages which we have received and should execute when the time comes.
    pub received: BinaryHeap<Message>,
    /// The collected metrics during the entire execution.
    pub metrics: NodeMetrics,
    /// The current metrics being collected for the current frame.
    pub current_metrics: Metrics,
    /// The shared storage across all nodes.
    pub storage: Arc<TypedStorage>,
    /// The already emitted events.
    pub emitted: FxHashMap<String, u128>,
    next_rid: usize,
    _clean_up: WithCleanUpDrop,
}

// When we're getting dropped some futures may still be pending and hold state which
// may contain a connection or listener, and their Drop will use the `with_node` function
// we will have an error if the current thread doesn't have a reference to the node that
// is being executed. So here we set the current node as the node that is getting executed.
impl Drop for NodeState {
    fn drop(&mut self) {
        hook_node(self as *mut NodeState);
    }
}

/// This is the last field of the [`NodeState`] that allows us to run a function
/// after other fields are dropped. So that we can remote the pointer to this node
/// from the thread_local and remote the dangling pointer which would remain otherwise.
struct WithCleanUpDrop;

impl Drop for WithCleanUpDrop {
    #[inline(always)]
    fn drop(&mut self) {
        hook_node(std::ptr::null_mut());
    }
}

// We're cool with moving `LocalPool` across different threads.
unsafe impl Sync for NodeState {}
unsafe impl Send for NodeState {}

#[derive(Default)]
pub struct ListenerState {
    /// The current ongoing accept future.
    pub accept: Option<DeferredFutureWaker<Option<AcceptResponse>>>,
    /// The queued connection requests. We enqueue these when there is
    /// not an active 'accept'.
    pub queue: VecDeque<(RemoteAddr, ResourceId)>,
}

pub struct AcceptResponse {
    pub local_rid: ResourceId,
    pub remote: RemoteAddr,
    pub remote_rid: ResourceId,
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, PartialOrd, Ord, Eq)]
pub struct ResourceId(pub(crate) usize);

pub enum Resource {
    PendingConnection {
        waker: DeferredFutureWaker<Result<ResourceId, ConnectError>>,
        queue: VecDeque<Vec<u8>>,
    },
    EstablishedConnection {
        recv: Option<DeferredFutureWaker<Option<Vec<u8>>>>,
        queue: VecDeque<Vec<u8>>,
    },
}

impl NodeState {
    /// Create the empty state of a node.
    pub fn new(storage: Arc<TypedStorage>, count_nodes: usize, node_id: usize) -> Self {
        Self {
            node_id,
            count_nodes,
            time: 0,
            spawn_pool: LocalPool::new(),
            resources: HashMap::with_capacity(16),
            listening: HashMap::default(),
            outgoing: Vec::new(),
            received: BinaryHeap::new(),
            metrics: NodeMetrics::default(),
            current_metrics: Metrics::default(),
            storage,
            emitted: FxHashMap::default(),
            next_rid: 0,
            _clean_up: WithCleanUpDrop,
        }
    }

    /// Consumes and returns the next resource id.
    #[inline(always)]
    fn get_rid(&mut self) -> ResourceId {
        let rid = self.next_rid;
        self.next_rid += 1;
        ResourceId(rid)
    }

    /// Returns the current time on the node.
    pub fn now(&mut self) -> u128 {
        let now = self.time;
        self.time += 1;
        now
    }

    /// Send a request to establish a connection with the given peer on the provided port number.
    ///
    /// Returns a future that will be resolved when the connection is established.
    pub fn connect(
        &mut self,
        remote: RemoteAddr,
        port: u16,
    ) -> (ResourceId, DeferredFuture<Result<ResourceId, ConnectError>>) {
        let rid = self.get_rid();
        let future = DeferredFuture::new();
        let resource = Resource::PendingConnection {
            waker: future.waker(),
            queue: VecDeque::default(),
        };

        let message = Message {
            sender: RemoteAddr(self.node_id),
            receiver: remote,
            time: std::cmp::Reverse(self.now()),
            detail: MessageDetail::Connect { port, rid },
        };

        self.resources.insert(rid, resource);
        self.outgoing.push(message);
        self.current_metrics.connections_requested += 1;

        (rid, future)
    }

    pub fn listen(&mut self, port: u16) {
        if self.listening.contains_key(&port) {
            panic!("Port {port} is already in used.");
        }

        self.listening.insert(port, ListenerState::default());
    }

    pub fn close_listener(&mut self, port: u16) {
        let mut state = self
            .listening
            .remove(&port)
            .unwrap_or_else(|| panic!("Port {port} is not being listened on"));

        if let Some(waker) = state.accept.take() {
            waker.wake(None);
        }

        for (addr, rid) in state.queue {
            self.refuse_connection(addr, rid);
        }
    }

    pub fn accept(&mut self, port: u16) -> DeferredFuture<Option<AcceptResponse>> {
        let listener_state = self.listening.get_mut(&port).expect("Illegal accept call.");

        assert!(
            listener_state.accept.is_none(),
            "Another accept call is still on-going."
        );

        if let Some((addr, rid)) = listener_state.queue.pop_front() {
            self.current_metrics.connections_accepted += 1;
            let res = self.accepted(addr, rid);
            DeferredFuture::resolved(Some(res))
        } else {
            let future = DeferredFuture::new();
            listener_state.accept = Some(future.waker());
            future
        }
    }

    pub fn send(&mut self, remote: RemoteAddr, rid: ResourceId, data: Vec<u8>) {
        self.current_metrics.msg_sent += 1;
        self.current_metrics.bytes_sent += data.len() as u64;

        let message = Message {
            sender: RemoteAddr(self.node_id),
            receiver: remote,
            time: std::cmp::Reverse(self.now()),
            detail: MessageDetail::Data {
                receiver_rid: rid,
                data,
            },
        };

        self.outgoing.push(message);
    }

    pub fn recv(&mut self, rid: ResourceId) -> DeferredFuture<Option<Vec<u8>>> {
        let resource = self
            .resources
            .get_mut(&rid)
            .expect("recv: Resource not found.");

        let (recv, queue) = if let Resource::EstablishedConnection { recv, queue } = resource {
            (recv, queue)
        } else {
            panic!("Invalid resource type.");
        };

        if let Some(msg) = queue.pop_front() {
            self.current_metrics.msg_processed += 1;
            self.current_metrics.bytes_processed += msg.len() as u64;
            DeferredFuture::resolved(Some(msg))
        } else {
            assert!(recv.is_none(), "Another recv is already in progress.");
            let future = DeferredFuture::<Option<Vec<u8>>>::new();
            *recv = Some(future.waker());
            future
        }
    }

    pub fn close_connection(&mut self, local_rid: ResourceId, addr: RemoteAddr, rid: ResourceId) {
        self.close_local_connection(local_rid);

        let message = Message {
            sender: RemoteAddr(self.node_id),
            receiver: addr,
            time: std::cmp::Reverse(self.now()),
            detail: MessageDetail::ConnectionClosed { receiver_rid: rid },
        };

        self.outgoing.push(message);
    }

    pub fn is_connection_open(&mut self, rid: ResourceId) -> bool {
        self.resources.contains_key(&rid)
    }

    pub fn sleep(&mut self, duration: u128) -> DeferredFuture<()> {
        let future = DeferredFuture::new();
        let waker = future.waker();

        let message = Message {
            sender: RemoteAddr(self.node_id),
            receiver: RemoteAddr(self.node_id),
            time: std::cmp::Reverse(self.now() + duration),
            detail: MessageDetail::WakeUp {
                waker: Ignored(waker),
            },
        };
        self.received.push(message);

        future
    }

    pub fn emit(&mut self, key: String) {
        if self.emitted.contains_key(&key) {
            println!("Event already emitted. node={} key={key}", self.node_id);
            return;
        }
        self.emitted.insert(key, self.time);
    }

    fn close_local_connection(&mut self, rid: ResourceId) {
        let resource = if let Some(resource) = self.resources.remove(&rid) {
            resource
        } else {
            return;
        };

        self.current_metrics.connections_closed += 1;

        let (mut recv, _queue) = if let Resource::EstablishedConnection { recv, queue } = resource {
            (recv, queue)
        } else {
            panic!("Invalid resource type.");
        };

        if let Some(waker) = recv.take() {
            waker.wake(None);
        }
    }

    fn process_message(&mut self, our_rid: ResourceId, data: Vec<u8>) {
        let resource = self
            .resources
            .get_mut(&our_rid)
            .expect("process_message: Resource not found.");

        let (recv, queue) = match resource {
            Resource::EstablishedConnection { recv, queue } => (recv, queue),
            Resource::PendingConnection { queue, .. } => {
                queue.push_back(data);
                return;
            },
        };

        if let Some(waker) = recv.take() {
            self.current_metrics.msg_processed += 1;
            self.current_metrics.bytes_processed += data.len() as u64;
            waker.wake(Some(data));
        } else {
            queue.push_back(data);
        }
    }

    fn accepted(&mut self, addr: RemoteAddr, remote_rid: ResourceId) -> AcceptResponse {
        let rid = self.get_rid();

        let resource = Resource::EstablishedConnection {
            recv: None,
            queue: VecDeque::new(),
        };

        let message = Message {
            sender: RemoteAddr(self.node_id),
            receiver: addr,
            time: std::cmp::Reverse(self.now()),
            detail: MessageDetail::ConnectionAccepted {
                sender_rid: rid,
                receiver_rid: remote_rid,
            },
        };

        self.resources.insert(rid, resource);
        self.outgoing.push(message);

        AcceptResponse {
            remote_rid,
            local_rid: rid,
            remote: addr,
        }
    }

    fn refuse_connection(&mut self, addr: RemoteAddr, remote_rid: ResourceId) {
        let message = Message {
            sender: RemoteAddr(self.node_id),
            receiver: addr,
            time: std::cmp::Reverse(self.now()),
            detail: MessageDetail::ConnectionRefused {
                receiver_rid: remote_rid,
            },
        };

        self.outgoing.push(message);
    }

    fn maybe_accept_new_connection(&mut self, port: u16, addr: RemoteAddr, rid: ResourceId) {
        let listener_state = if let Some(s) = self.listening.get_mut(&port) {
            s
        } else {
            self.current_metrics.connections_refused += 1;
            return self.refuse_connection(addr, rid);
        };

        if let Some(waker) = listener_state.accept.take() {
            self.current_metrics.connections_accepted += 1;
            let res = self.accepted(addr, rid);
            waker.wake(Some(res));
        } else {
            listener_state.queue.push_back((addr, rid));
        }
    }

    fn resolve_connection(
        &mut self,
        our_rid: ResourceId,
        result: Result<ResourceId, ConnectError>,
    ) {
        let resource = self.resources.remove(&our_rid).unwrap();

        let queue = if let Resource::PendingConnection { waker, queue } = resource {
            waker.wake(result);
            queue
        } else {
            VecDeque::new()
        };

        if result.is_ok() {
            self.resources.insert(
                our_rid,
                Resource::EstablishedConnection { recv: None, queue },
            );
        }
    }

    pub fn is_stalled(&self) -> bool {
        !matches!(self.received.peek(), Some(msg) if msg.time.0 <= self.time)
    }

    pub fn run_until_stalled(&mut self) {
        while !self.is_stalled() {
            let msg = self.received.pop().unwrap();

            // eprintln!("\t>current {}: {:?}", self.node_id, msg);

            match msg.detail {
                MessageDetail::Connect { port, rid } => {
                    self.maybe_accept_new_connection(port, msg.sender, rid);
                },
                MessageDetail::ConnectionAccepted {
                    sender_rid,
                    receiver_rid,
                } => {
                    self.resolve_connection(receiver_rid, Ok(sender_rid));
                },
                MessageDetail::ConnectionRefused { receiver_rid } => {
                    self.current_metrics.connections_failed += 1;
                    self.resolve_connection(receiver_rid, Err(ConnectError::RemoteIsDown));
                },
                MessageDetail::ConnectionClosed { receiver_rid: rid } => {
                    self.close_local_connection(rid);
                },
                MessageDetail::Data { receiver_rid, data } => {
                    self.current_metrics.msg_received += 1;
                    self.current_metrics.bytes_received += data.len() as u64;
                    self.process_message(receiver_rid, data);
                },
                MessageDetail::WakeUp { waker } => {
                    waker.wake(());
                },
            }
        }

        self.spawn_pool.run_until_stalled();
    }
}
