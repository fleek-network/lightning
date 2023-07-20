use std::{
    cell::RefCell,
    collections::{BTreeSet, HashMap, VecDeque},
};

use futures::executor::LocalPool;
use fxhash::FxHashMap;

use crate::{
    api::{ConnectError, RemoteAddr},
    future::{DeferredFuture, DeferredFutureWaker},
    message::{Message, MessageDetail},
};

thread_local! {
    static NODES: RefCell<*mut NodeState> = RefCell::new(std::ptr::null_mut());
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
    pub received: BTreeSet<Message>,
    next_rid: usize,
}

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
    },
    EstablishedConnection {
        recv: Option<DeferredFutureWaker<()>>,
        queue: VecDeque<Vec<u8>>,
    },
}

impl NodeState {
    /// Create the empty state of a node.
    pub fn new(count_nodes: usize, node_id: usize) -> Self {
        Self {
            node_id,
            count_nodes,
            time: 0,
            spawn_pool: LocalPool::new(),
            resources: HashMap::with_capacity(16),
            listening: HashMap::default(),
            outgoing: Vec::new(),
            received: BTreeSet::new(),
            next_rid: 0,
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
        self.time
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
        };

        let message = Message {
            sender: RemoteAddr(self.node_id),
            receiver: remote,
            time: self.now(),
            detail: MessageDetail::Connect { port, rid },
        };

        self.resources.insert(rid, resource);
        self.outgoing.push(message);

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
            let res = self.accepted(addr, rid);
            DeferredFuture::resolved(Some(res))
        } else {
            let future = DeferredFuture::new();
            listener_state.accept = Some(future.waker());
            future
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
            time: self.now(),
            detail: MessageDetail::ConnectionAccepted {
                source_rid: rid,
                remote_rid,
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
            time: self.now(),
            detail: MessageDetail::ConnectionRefused {
                source_rid: remote_rid,
            },
        };

        self.outgoing.push(message);
    }

    fn maybe_accept_new_connection(&mut self, port: u16, addr: RemoteAddr, rid: ResourceId) {
        let listener_state = if let Some(s) = self.listening.get_mut(&port) {
            s
        } else {
            return self.refuse_connection(addr, rid);
        };

        if let Some(waker) = listener_state.accept.take() {
            let res = self.accepted(addr, rid);
            waker.wake(Some(res));
        } else {
            listener_state.queue.push_back((addr, rid));
        }
    }

    fn resolve_connection(
        &mut self,
        source_rid: ResourceId,
        result: Result<ResourceId, ConnectError>,
    ) {
        let resource = self.resources.remove(&source_rid).unwrap();

        if result.is_ok() {
            self.resources.insert(
                source_rid,
                Resource::EstablishedConnection {
                    recv: None,
                    queue: VecDeque::new(),
                },
            );
        }

        if let Resource::PendingConnection { waker } = resource {
            waker.wake(result);
        }
    }

    pub fn run_until_stalled(&mut self) {
        while let Some(msg) = self.received.pop_first() {
            if msg.time > self.time {
                self.received.insert(msg);
                break;
            }

            println!("current {}: {:?}", self.node_id, msg);

            match msg.detail {
                MessageDetail::Connect { port, rid } => {
                    self.maybe_accept_new_connection(port, msg.sender, rid);
                },
                MessageDetail::ConnectionAccepted {
                    source_rid,
                    remote_rid,
                } => {
                    self.resolve_connection(source_rid, Ok(remote_rid));
                },
                MessageDetail::ConnectionRefused { source_rid } => {
                    self.resolve_connection(source_rid, Err(ConnectError::RemoteIsDown));
                },
                MessageDetail::ConnectionClosed { .. } => {
                    // todo
                },
                MessageDetail::Data { .. } => {
                    // todo
                },
            }
        }

        self.spawn_pool.run_until_stalled();
    }
}
