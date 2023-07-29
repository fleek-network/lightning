use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
    time::Duration,
};

use fleek_crypto::NodeNetworkingPublicKey;
use thiserror::Error;
use tokio::{
    net::UdpSocket,
    select,
    sync::{
        mpsc::{Receiver, Sender},
        oneshot,
    },
    time,
};

use crate::{
    bucket::MAX_BUCKETS,
    distance::{self, Distance},
    query::{Message, MessagePayload, NodeInfo, Query, Response},
    socket,
    table::{TableKey, TableQuery},
};

/// Kademlia's lookup procedure.
pub async fn lookup(mut lookup: LookupTask) -> Result<Vec<NodeInfo>, LookUpError> {
    // Get initial K closest nodes from our local table.
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
    let nodes = rx
        .await
        .map_err(|e| LookUpError(e.to_string()))?
        .map_err(|e| {
            tracing::error!("failed to get closest nodes: {e:?}");
            LookUpError(e.to_string())
        })?
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
    // Timeout for every round.
    let mut timeout = time::interval(Duration::from_secs(4));
    loop {
        // Pending is empty when a round has finished.
        if pending.is_empty() {
            for node in lookup
                .closest_nodes
                .pickout(MAX_BUCKETS, 3, |node| node.status == Status::Initial)
            {
                let message = Message {
                    // Todo: Generate random transaction ID.
                    // Message: Maybe we need to add breadcrumb to message.
                    id: lookup.task_id,
                    payload: MessagePayload::Query(Query::Find {
                        sender_id: lookup.local_key,
                        target: lookup.target,
                    }),
                };
                let bytes = bincode::serialize(&message).unwrap();
                socket::send_to(&lookup.socket, bytes.as_slice(), node.inner.address)
                    .await
                    .unwrap();
                pending.insert(node.inner.key.0, node.inner);
            }
            if !pending.is_empty() {
                // We have found closer nodes so we start another round.
                timeout.reset();
            }
        }

        select! {
            // We want timeout to be polled first to control round switches
            // and avoid network channel always being ready.
            biased;
            // Timeout for round.
            _ = timeout.tick() => {
                if pending.is_empty() {
                    // This can't be empty at this point because
                    // it should have been filled at the start of the loop.
                    break;
                }
                for (key, node) in pending.into_iter() {
                    late.insert(key, node);
                }
                pending = HashMap::new();
                continue;
            }
            // Incoming K nodes from peers.
            response = lookup.main_rx.recv() => {
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
        }
    }

    Ok(lookup
        .closest_nodes
        .into_nodes()
        .map(|lookup_node| lookup_node.inner)
        .take(MAX_BUCKETS)
        .collect())
}

#[derive(Debug, Error)]
#[error("lookup procedure failed: {0}")]
pub struct LookUpError(String);

#[derive(Clone)]
pub struct LookupHandle(pub Sender<Response>);

pub struct LookupTask {
    task_id: u64,
    // Closest nodes.
    closest_nodes: LookupMap<LookupNode>,
    // Our node's local key.
    local_key: NodeNetworkingPublicKey,
    // Target that we're looking for.
    target: TableKey,
    // Send queries to table server.
    table_tx: Sender<TableQuery>,
    // Receive messages from server.
    main_rx: Receiver<Response>,
    // Socket to send queries over the network.
    socket: Arc<UdpSocket>,
}

impl LookupTask {
    pub fn new(
        task_id: u64,
        local_key: NodeNetworkingPublicKey,
        target: TableKey,
        table_tx: Sender<TableQuery>,
        main_rx: Receiver<Response>,
        socket: Arc<UdpSocket>,
    ) -> Self {
        Self {
            task_id,
            closest_nodes: LookupMap::new(target),
            local_key,
            target,
            table_tx,
            main_rx,
            socket,
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
    fn pickout<P>(&mut self, k: usize, alpha: usize, mut predicate: P) -> Vec<V>
    where
        P: FnMut(&V) -> bool,
    {
        let mut pruned = Vec::with_capacity(alpha);
        let mut count = 0;
        for (distance, value) in self.closest.iter().take(k) {
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

    fn into_nodes(self) -> impl Iterator<Item = V> {
        self.closest.into_values()
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
