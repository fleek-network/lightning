use std::{collections::HashMap, sync::Arc};

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
    bucket::MAX_BUCKETS,
    distance::DistanceMap,
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
                        closest_nodes: DistanceMap::new(request.target),
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

    lookup.closest_nodes.add_nodes(nodes);

    let mut pending = HashMap::new();
    loop {
        let mut closer_exists = false;
        // Pending is empty when a round has finished.
        if pending.is_empty() {
            for node in lookup
                .closest_nodes
                .nodes_mut()
                .filter(|node| node.status == Status::Initial || node.status == Status::Responded)
                .take(MAX_BUCKETS)
                .filter(|node| node.status == Status::Initial)
                .take(3)
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
                node.status = Status::Queried;
                pending.insert(node.inner.key, node.inner.key);
                closer_exists = true;
            }
        }

        // Maybe we should put a timer in case we never get anything from the network.
        match lookup.server_rx.recv().await {
            None => panic!("Server dropped the network response channel"),
            Some(response) => {
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
                lookup.closest_nodes.add_nodes(nodes);
            },
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

#[derive(PartialEq)]
enum Status {
    Initial,
    Queried,
    Responded,
}

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
    closest_nodes: DistanceMap<LookupNode>,
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
