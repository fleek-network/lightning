use std::{
    cell::RefCell,
    collections::{HashMap, VecDeque},
};

use futures::executor::LocalPool;
use fxhash::FxHashMap;

use crate::{
    api::{ConnectError, RemoteAddr},
    future::{DeferredFuture, DeferredFutureWaker},
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
    pub received: VecDeque<Message>,
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

pub struct Message {
    pub time: u128,
    pub sender: RemoteAddr,
    pub detail: MessageDetail,
}

pub enum MessageDetail {
    Connect {
        remote: RemoteAddr,
        port: u16,
        rid: ResourceId,
    },
    Connected {
        source: RemoteAddr,
        source_rid: ResourceId,
        remote_rid: ResourceId,
    },
    ConnectionRefused {
        source: RemoteAddr,
        source_rid: ResourceId,
    },
    ConnectionClosed {
        remote: RemoteAddr,
        rid: ResourceId,
    },
    Data {
        data: Vec<u8>,
        remote: RemoteAddr,
        remote_rid: ResourceId,
    },
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, PartialOrd, Ord, Eq)]
pub struct ResourceId(usize);

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
            received: VecDeque::new(),
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
            time: self.now(),
            detail: MessageDetail::Connect { remote, port, rid },
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

    pub fn accept(&mut self, port: u16) -> DeferredFuture<Option<AcceptResponse>> {
        let future = DeferredFuture::new();

        let listener_state = self.listening.get_mut(&port).expect("Illegal accept call.");

        if listener_state.accept.replace(future.waker()).is_some() {
            panic!("Another accept call is still on-going.");
        }

        if let Some(_conn) = listener_state.queue.pop_front() {
            // todo
        }

        future
    }

    pub fn run_until_stalled(&mut self) {
        self.spawn_pool.run_until_stalled();
    }
}
