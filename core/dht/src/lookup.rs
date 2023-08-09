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
    bucket::MAX_BUCKET_SIZE,
    distance::{self, Distance},
    query::{Message, MessageType, NodeInfo, Query, Response},
    socket,
    table::{TableCommand, TableKey},
};

/// Kademlia's lookup procedure.
pub async fn lookup(mut lookup: LookupTask) -> Result<LookupResult, LookUpError> {
    // Get initial K closest nodes from our local table.
    let (tx, rx) = oneshot::channel();
    let table_query = TableCommand::ClosestNodes {
        target: lookup.target,
        tx,
    };
    lookup
        .table_tx
        .send(table_query)
        .await
        .expect("table worker not to drop the channel");
    let nodes = rx
        .await
        .expect("table worker not to drop the channel")
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
    let mut timeout = time::interval(Duration::from_secs(2));
    loop {
        // Pending is empty when a round has finished.
        if pending.is_empty() {
            for node in lookup
                .closest_nodes
                .pickout(MAX_BUCKET_SIZE, 3, |node| node.status == Status::Initial)
            {
                let id = rand::random();
                let payload = bincode::serialize(&Query::Find {
                    find_value: lookup.find_value_lookup,
                    target: lookup.target,
                })
                .expect("query to be valid");
                let message = Message {
                    ty: MessageType::Query,
                    token: id,
                    id: lookup.id,
                    sender_key: lookup.local_key,
                    payload,
                };
                let bytes = bincode::serialize(&message).expect("query to be valid");
                socket::send_to(&lookup.socket, bytes.as_slice(), node.inner.address)
                    .await
                    .map_err(|e| LookUpError(e.to_string()))?;
                pending.insert(
                    node.inner.key,
                    PendingResponse {
                        node: node.inner,
                        id,
                    },
                );
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
                    // This can't be empty at this point if we found closer nodes in the last round
                    // because it should have been filled at the start of the loop.
                    break;
                }
                for (key, node) in pending.into_iter() {
                    late.insert(key, node);
                }
                pending = HashMap::new();
                continue;
            }
            // Incoming K nodes from peers.
            message = lookup.main_rx.recv() => {
                let response_event = message.expect("dispatcher worker not to drop the channel");
                let sender_key = response_event.sender_key;
                let response_id = response_event.id;
                let response = response_event.response;
                if pending.contains_key(&sender_key) || late.contains_key(&sender_key) {
                    // Validate id in response.
                    let expected_id = match pending.get(&sender_key) {
                        Some(pending) => pending.id,
                        // It's fine to use unwrap here because of the of condition.
                        None => late.get(&sender_key).unwrap().id,
                    };

                    // If the id does not match, we ignore this response.
                    if expected_id != response_id {
                        tracing::trace!("expected id {expected_id} but received instead {response_id}");
                        continue;
                    }

                    // If this is look up is a find a value, we check if the value is in the response.
                    if lookup.find_value_lookup && response.value.is_some() {
                        return Ok(LookupResult::Value(response.value));
                    }

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
                    let node = match pending.remove(&sender_key) {
                        Some(pending) => pending.node,
                        // It's fine to use unwrap here because of the of condition.
                        None => late.remove(&sender_key).unwrap().node,
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
                    tracing::warn!("received unsolicited list of nodes from {sender_key:?}");
                }
            }
        }
    }

    // Close channel to indicate to dispatcher that this task is done.
    lookup.main_rx.close();

    if lookup.find_value_lookup {
        Ok(LookupResult::Value(None))
    } else {
        Ok(LookupResult::Nodes(
            lookup
                .closest_nodes
                .into_nodes()
                .map(|lookup_node| lookup_node.inner)
                .take(MAX_BUCKET_SIZE)
                .collect(),
        ))
    }
}

#[derive(Debug, Error)]
#[error("lookup procedure failed: {0}")]
pub struct LookUpError(String);

pub struct LookupTask {
    // Task identifier.
    id: u64,
    // True if this is a `find value` look up.
    find_value_lookup: bool,
    // Closest nodes.
    closest_nodes: LookupMap<LookupNode>,
    // Our node's local key.
    local_key: NodeNetworkingPublicKey,
    // Target that we're looking for.
    target: TableKey,
    // Send queries to table server.
    table_tx: Sender<TableCommand>,
    // Receive events about responses received from the network.
    main_rx: Receiver<ResponseEvent>,
    // Socket to send queries over the network.
    socket: Arc<UdpSocket>,
}

impl LookupTask {
    pub fn new(
        task_id: u64,
        find_value_lookup: bool,
        local_key: NodeNetworkingPublicKey,
        target: TableKey,
        table_tx: Sender<TableCommand>,
        main_rx: Receiver<ResponseEvent>,
        socket: Arc<UdpSocket>,
    ) -> Self {
        Self {
            id: task_id,
            find_value_lookup,
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

pub enum LookupResult {
    Nodes(Vec<NodeInfo>),
    Value(Option<Vec<u8>>),
}

#[derive(Debug)]
struct PendingResponse {
    node: NodeInfo,
    id: u64,
}

#[derive(Debug)]
pub struct ResponseEvent {
    pub id: u64,
    pub sender_key: NodeNetworkingPublicKey,
    pub response: Response,
}
