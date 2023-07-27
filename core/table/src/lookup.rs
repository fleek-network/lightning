use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
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
};
use tokio_util::time::DelayQueue;

use crate::{
    bucket::{HASH_LEN, MAX_BUCKET_SIZE},
    distance,
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
    let closest_nodes = match rx.await.expect("Table to not drop the channel") {
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

    let mut query_list = QueryList {
        inner: BTreeMap::new(),
        min_distance_seen: HASH_LEN * 8,
        target: lookup.target,
    };
    query_list.add_nodes(closest_nodes);
    loop {
        // Todo: Choose alpha.
        for querying in query_list.nodes_mut().take(MAX_BUCKET_SIZE) {
            if !querying.queried {
                let message = Message {
                    id: 0,
                    payload: MessagePayload::Query(Query::Find {
                        sender_id: lookup.local_id,
                        target: lookup.target,
                    }),
                };
                let bytes = bincode::serialize(&message).unwrap();
                socket::send_to(&lookup.socket, bytes.as_slice(), querying.node.address)
                    .await
                    .unwrap();
                querying.queried = true;
            }
        }
        // Maybe we should put a timer in case we never get anything from the network.
        match lookup.server_rx.recv().await {
            None => panic!("Server dropped the network response channel"),
            Some(response) => {
                match response {
                    Response::NodeInfo(closest_nodes) => {
                        let new_closer_nodes = query_list.add_nodes(closest_nodes);
                        if !new_closer_nodes {
                            break;
                        }
                    },
                    // Todo: Fix this.
                    Response::Pong => unreachable!(),
                }
            },
        }
    }
    lookup
        .server_tx
        .send(Ok(query_list
            .nodes()
            .next()
            .map(|state| state.node.key.0)
            .unwrap()))
        .await
        .unwrap();
}

struct QueryList {
    target: TableKey,
    min_distance_seen: usize,
    inner: BTreeMap<usize, QueryingNode>,
}

impl QueryList {
    /// Returns true if at least one of the nodes added is closer to target,
    /// otherwise, it returns false.
    pub fn add_nodes(&mut self, nodes: Vec<NodeInfo>) -> bool {
        for node in nodes {
            let distance = distance::leading_zero_bits(&node.key.0, &self.target);
            self.inner.insert(
                distance,
                QueryingNode {
                    node,
                    queried: false,
                },
            );
        }
        let current_distance = match self.inner.first_key_value() {
            Some((distance, _)) => *distance,
            None => {
                panic!("empty query list");
            },
        };
        if current_distance < self.min_distance_seen {
            self.min_distance_seen = current_distance;
            true
        } else {
            false
        }
    }

    fn nodes(&self) -> impl Iterator<Item = &QueryingNode> {
        self.inner.values()
    }

    fn nodes_mut(&mut self) -> impl Iterator<Item = &mut QueryingNode> {
        self.inner.values_mut()
    }
}

pub struct QueryingNode {
    node: NodeInfo,
    queried: bool,
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
