use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
    time::Duration,
};

use fleek_crypto::NodeNetworkingPublicKey;
use thiserror::Error;
use tokio::{
    macros::support::poll_fn,
    net::UdpSocket,
    select,
    sync::{
        mpsc::{self, Receiver, Sender},
        oneshot,
    },
    time,
};
use tokio_util::time::DelayQueue;

use crate::{
    distance::{self, Distance},
    query::{Message, MessagePayload, NodeInfo, Query, Response},
    socket,
    table::{TableKey, TableQuery},
};

pub async fn _start_server(
    // Send lookup responses.
    response_tx: Sender<LookupResponse>,
    // Receive lookup requests.
    mut request_rx: Receiver<LookupRequest>,
    // Receive responses from the network routed by handler.
    mut network_rx: Receiver<Response>,
    // Send queries to table server.
    table_tx: Sender<TableQuery>,
    // Socket to send queries over the network.
    socket: Arc<UdpSocket>,
    // Local node's public key.
    key: NodeNetworkingPublicKey,
) {
    let main_server = LookUpServer::new(response_tx);
    // Todo: add delay queue to server.
    let mut expired: DelayQueue<u64> = Default::default();
    let (server_tx, mut server_rx) = mpsc::channel(10000);
    loop {
        let mut server = main_server.clone();
        select! {
            request = request_rx.recv() => {
                let request = request.unwrap();
                if server.pending.get(&request.target).is_none() {
                    let (task_tx, task_rx) = mpsc::channel(10000);
                    server.pending.insert(
                        request.target,
                        LookupHandle { _request: request.clone(), _task_tx: task_tx }
                    );
                    let lookup = LookUp {
                        local_id: key,
                        target: request.target,
                        table_tx: table_tx.clone(),
                        server_tx: server_tx.clone(),
                        server_rx:task_rx,
                        _inflight: HashMap::new(),
                        socket: socket.clone(),
                        closest_nodes: LookupMap::new(request.target),
                    };
                    tokio::spawn(run_lookup(lookup));
                }
            }
            _network_response = network_rx.recv() => {}
            _exp = poll_fn(|cx| expired.poll_expired(cx)) => {}
            _update = server_rx.recv() => {}
        }
    }
}

/// Kademlia's lookup procedure.
async fn run_lookup(mut lookup: LookUp) {
    // Get initial K closest nodes.
    let (tx, rx) = oneshot::channel();
    let table_query = TableQuery::ClosestNodes {
        target: lookup.target,
        tx,
    };
    lookup
        .table_tx
        .send(table_query)
        .await
        .expect("Table to not drop the channel");
    let nodes = match rx.await.expect("Table to not drop the channel") {
        Ok(nodes) => nodes,
        Err(e) => {
            tracing::error!("failed to get closest nodes: {e:?}");
            lookup
                .server_tx
                .send(Err(LookUpError))
                .await
                .expect("Server to not drop the channel");
            return;
        },
    };

    let nodes = nodes
        .into_iter()
        .map(|node| {
            (
                node.key.0,
                LookupNode {
                    inner: node,
                    status: Status::Initial,
                },
            )
        })
        .collect();

    lookup.closest_nodes.insert_new_entries(nodes);

    // Nodes on which we are waiting for a response.
    let mut pending = HashMap::new();
    // Nodes that didn't send a response in time.
    let mut late = HashMap::new();
    let mut timeout = time::interval(Duration::from_secs(4));
    loop {
        let mut closer_exists = false;
        // Pending is empty when a round has finished.
        if pending.is_empty() {
            for node in lookup
                .closest_nodes
                .prune(3, |node| node.status == Status::Initial)
            {
                let message = Message {
                    // Todo: Generate random transaction ID.
                    id: 0,
                    payload: MessagePayload::Query(Query::Find {
                        sender_id: lookup.local_id,
                        target: lookup.target,
                    }),
                };
                let bytes = bincode::serialize(&message).unwrap();
                socket::send_to(&lookup.socket, bytes.as_slice(), node.inner.address)
                    .await
                    .unwrap();
                pending.insert(node.inner.key.0, node.inner);
                closer_exists = true;
            }
            timeout.reset();
        }

        select! {
            response = lookup.server_rx.recv() => {
                let response = response.unwrap();
                let sender_id = response.sender_id;
                if pending.contains_key(&sender_id) || late.contains_key(&sender_id) {
                    let nodes = response
                        .nodes
                        .into_iter()
                        .map(|node| {
                            (
                                node.key.0,
                                LookupNode {
                                    inner: node,
                                    status: Status::Initial,
                                },
                            )
                        })
                        .collect();

                    // Add new nodes to closest nodes list.
                    lookup.closest_nodes.insert_new_entries(nodes);

                    // Remove sender from pending list.
                    let node = match pending.remove(&sender_id) {
                        Some(node) => node,
                        None => late.remove(&sender_id).unwrap(),
                    };

                    // Put this node back to closest nodes list.
                    assert!(lookup.closest_nodes.update(
                        node.key.0,
                        LookupNode {
                            inner: node,
                            status: Status::Responded
                        }
                    ).is_none());
                } else {
                    tracing::warn!("received unsolicited list of nodes from {sender_id:?}");
                }
            }
            _ = timeout.tick() => {
                for (key, node) in pending.into_iter() {
                    late.insert(key, node);
                }
                pending = HashMap::new();
                continue;
            }
        }

        if !closer_exists && pending.is_empty() {
            lookup
                .server_tx
                .send(Ok(lookup
                    .closest_nodes
                    .nodes()
                    .next()
                    .map(|node| node.inner.key.0)
                    .unwrap()))
                .await
                .unwrap();
            break;
        }
    }
}

#[derive(Clone, PartialEq)]
enum Status {
    Initial,
    Responded,
}

#[derive(Clone)]
pub struct LookupNode {
    inner: NodeInfo,
    status: Status,
}

#[derive(Debug, Error)]
#[error("Lookup procedure failed")]
pub struct LookUpError;

#[derive(Clone)]
pub struct LookupHandle {
    _request: LookupRequest,
    _task_tx: Sender<Response>,
}

#[derive(Clone)]
pub struct LookupRequest {
    pub target: TableKey,
}

pub struct LookupResponse {
    pub target: TableKey,
    pub nodes: Vec<NodeInfo>,
}

pub struct LookUp {
    // Closest nodes.
    closest_nodes: LookupMap<LookupNode>,
    // Our node's local key.
    local_id: NodeNetworkingPublicKey,
    // Target that we're looking for.
    target: TableKey,
    // Send queries to table server.
    table_tx: Sender<TableQuery>,
    // Channel to send messages to server.
    // At the moment, we just tell the server we're done.
    server_tx: Sender<Result<TableKey, LookUpError>>,
    // Receive messages from server.
    server_rx: Receiver<Response>,
    // Inflight requests.
    _inflight: HashMap<u64, LookUp>,
    // Socket to send queries over the network.
    socket: Arc<UdpSocket>,
}

#[derive(Clone)]
pub struct LookUpServer {
    // Ongoing lookups.
    pending: HashMap<TableKey, LookupHandle>,
    // Send lookup responses.
    _response_tx: Sender<LookupResponse>,
}

impl LookUpServer {
    fn new(response_tx: Sender<LookupResponse>) -> Self {
        Self {
            _response_tx: response_tx,
            pending: HashMap::new(),
        }
    }
}

struct LookupMap<V> {
    target: Arc<TableKey>,
    closest: BTreeMap<Distance, V>,
}

impl<V: Clone> LookupMap<V> {
    fn new(target: TableKey) -> Self {
        Self {
            closest: BTreeMap::new(),
            target: Arc::new(target),
        }
    }
    /// Inserts new node entries. If an entry already exists in the map,
    /// the entry will not be updated and new value is ignored.
    fn insert_new_entries(&mut self, nodes: Vec<(TableKey, V)>) {
        for (key, value) in nodes {
            let distance = distance::distance(&self.target, &key);
            if self.closest.contains_key(&distance) {
                continue;
            }
            self.closest.insert(distance, value);
        }
    }

    /// Inserts a key-value pair.
    /// If the map did not have this key present, None is returned.
    /// If the map did have this key present, the value is updated, and the old value is returned.
    fn update(&mut self, key: TableKey, value: V) -> Option<V> {
        let distance = distance::distance(&self.target, &key);
        self.closest.insert(distance, value)
    }

    /// Removes and returns the first alpha values from the map for those values
    /// such that `predicate(value) == true`.
    fn prune<P>(&mut self, alpha: usize, mut predicate: P) -> Vec<V>
    where
        P: FnMut(&V) -> bool,
    {
        let mut pruned = Vec::with_capacity(alpha);
        let mut count = 0;
        for (distance, value) in self.closest.iter() {
            if count >= alpha {
                break;
            }
            if predicate(value) {
                count += 1;
                pruned.push((*distance, value.clone()))
            }
        }
        for (distance, _) in pruned.iter() {
            self.closest.remove(distance);
        }
        pruned.into_iter().map(|(_, v)| v).collect()
    }

    fn nodes(&self) -> impl Iterator<Item = &V> {
        self.closest.values()
    }
}
