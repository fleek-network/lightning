use std::collections::{BTreeMap, HashMap};
use std::marker::PhantomData;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use anyhow::Result;
use async_trait::async_trait;
use bytes::Bytes;
use fleek_crypto::NodePublicKey;
use infusion::c;
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::types::NodeIndex;
use lightning_interfaces::{ApplicationInterface, SyncQueryRunnerInterface};
use tokio::sync::mpsc::Receiver;

use crate::network::UnreliableTransport;
use crate::pool::worker::FindQueryResponse;
use crate::table::bucket::MAX_BUCKET_SIZE;
use crate::table::distance;
use crate::table::distance::Distance;
use crate::table::server::TableKey;
use crate::{network, table};

pub struct Context {
    /// Task id.
    pub id: u32,
    /// Queue for receiving messages from the network.
    pub queue: Receiver<FindQueryResponse>,
}

#[async_trait]
pub trait LookupInterface: Clone + Send + Sync + 'static {
    async fn lookup_contact(&self, key: TableKey, ctx: Context) -> Result<Vec<NodeIndex>>;

    async fn lookup_value(&self, key: TableKey, ctx: Context) -> Result<Option<Bytes>>;
}

pub struct Looker<C: Collection, T> {
    us: NodeIndex,
    table: table::Client,
    sync_query: c![C::ApplicationInterface::SyncExecutor],
    socket: T,
    _marker: PhantomData<C>,
}

impl<C, T> Clone for Looker<C, T>
where
    C: Collection,
    T: UnreliableTransport,
{
    fn clone(&self) -> Self {
        Self {
            us: self.us,
            table: self.table.clone(),
            sync_query: self.sync_query.clone(),
            socket: self.socket.clone(),
            _marker: PhantomData,
        }
    }
}

impl<C, T> Looker<C, T>
where
    C: Collection,
    T: UnreliableTransport,
{
    fn _new(
        us: NodeIndex,
        table: table::Client,
        socket: T,
        sync_query: c!(C::ApplicationInterface::SyncExecutor),
    ) -> Self {
        Self {
            us,
            sync_query,
            table,
            socket,
            _marker: PhantomData,
        }
    }

    // Todo: this task needs to be broken down and cleaned up.
    // Todo: send events to table.
    pub async fn lookup(
        &self,
        key: TableKey,
        is_content: bool,
        mut ctx: Context,
    ) -> Result<(Vec<NodeIndex>, Bytes)> {
        let mut closest_nodes = LookupMap::<LookupNode>::new(key);

        // Get initial K closest nodes from our local table.
        let nodes = self
            .table
            .closest_contacts(key)
            .await?
            .into_iter()
            .filter_map(|index| {
                let pk = self.sync_query.index_to_pubkey(index)?;
                Some((
                    pk.0,
                    LookupNode {
                        key: pk,
                        index,
                        last_responded: None,
                        status: Status::Initial,
                    },
                ))
            })
            .collect();

        closest_nodes.insert_new_entries(nodes);

        // Nodes on which we are waiting for a response.
        let mut pending = HashMap::new();
        // Nodes that didn't send a response in time.
        let mut late = HashMap::new();
        // Timeout for every round.
        let mut timeout = tokio::time::interval(Duration::from_secs(2));
        loop {
            // Pending is empty when a round has finished.
            if pending.is_empty() {
                for node in
                    closest_nodes.pickout(MAX_BUCKET_SIZE, 3, |node| node.status == Status::Initial)
                {
                    let token = rand::random();
                    let message = if is_content {
                        network::find_value(ctx.id, token, self.us, key)
                    } else {
                        network::find_node(ctx.id, token, self.us, key)
                    };
                    self.socket.send(message, node.index).await?;

                    pending.insert(node.key, PendingResponse { node, token });
                }
                if !pending.is_empty() {
                    // We have found closer nodes so we start another round.
                    timeout.reset();
                }
            }

            tokio::select! {
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
                message = ctx.queue.recv() => {
                    let response_event = message.expect("handler worker not to drop the channel");
                    // Todo: Ignore if not in state.
                    let sender_key = self.sync_query.index_to_pubkey(response_event.from).unwrap();
                    let response_token = response_event.token;
                    let contacts = response_event.contacts;
                    let content = response_event.content;
                    // let response = response_event.response;
                    if pending.contains_key(&sender_key) || late.contains_key(&sender_key) {
                        // Validate id in response.
                        let expected_token = match pending.get(&sender_key) {
                            Some(pending) => pending.token,
                            // It's fine to use unwrap here because of the of condition.
                            None => late.get(&sender_key).unwrap().token,
                        };

                        // If the id does not match, we ignore this response.
                        if expected_token != response_token {
                            tracing::trace!("expected id {expected_token} but received instead {response_token}");
                            continue;
                        }

                        // Set the last_responded timestamp of the sender node.
                        let _timestamp = SystemTime::now()
                                .duration_since(SystemTime::UNIX_EPOCH)
                                .unwrap()
                                .as_millis() as u64;

                        let nodes: Vec<(TableKey, LookupNode)> =contacts
                            .into_iter()
                            .filter(|index| {
                                if let Some(key) = self.sync_query.index_to_pubkey(*index) {
                                    !pending.contains_key(&key)
                                    && !late.contains_key(&key)
                                    && self.sync_query
                                        .index_to_pubkey(self.us)
                                        .map(|our_key| key != our_key)
                                        .unwrap_or(false)
                                } else {
                                    false
                                }
                            })
                            .filter_map(|index| {
                                let key = self.sync_query.index_to_pubkey(index)?;
                                Some((
                                    key.0,
                                    LookupNode {
                                        key,
                                        index,
                                        last_responded: None,
                                        status: Status::Initial,
                                    },
                                ))
                            })
                            .collect();

                        // Add new nodes to closest nodes list.
                        closest_nodes.insert_new_entries(nodes);

                        // Remove sender from pending list.
                        let node = match pending.remove(&sender_key) {
                            Some(pending) => pending.node,
                            // It's fine to use unwrap here because of the of condition.
                            None => late.remove(&sender_key).unwrap().node,
                        };

                        // If this is look up is a find a value, we check if the value is in the response.
                        if is_content && !content.is_empty() {
                            let mut closest_nodes_seen = vec![node.index];
                            closest_nodes_seen.extend(closest_nodes
                                .into_nodes()
                                .map(|lookup_node| lookup_node.index)
                                .take(MAX_BUCKET_SIZE));

                            return Ok((closest_nodes_seen, content));
                        }

                        // Put this node back to closest nodes list.
                        assert!(closest_nodes.update(
                            node.key.0,
                            LookupNode {
                                key: node.key,
                                index: node.index,
                                last_responded: node.last_responded,
                                status: Status::Responded
                            }
                        ).is_none());
                    } else {
                        tracing::warn!("received unsolicited list of nodes from {sender_key:?}");
                    }
                }
            }
        }

        let closest_nodes_seen = closest_nodes
            .into_nodes()
            .map(|lookup_node| lookup_node.index)
            .take(MAX_BUCKET_SIZE)
            .collect();

        Ok((closest_nodes_seen, Bytes::new()))
    }
}

#[async_trait]
impl<C, T> LookupInterface for Looker<C, T>
where
    C: Collection,
    T: UnreliableTransport,
{
    async fn lookup_contact(&self, key: TableKey, ctx: Context) -> Result<Vec<NodeIndex>> {
        self.lookup(key, false, ctx)
            .await
            .map(|(contacts, _)| contacts)
    }

    async fn lookup_value(&self, key: TableKey, ctx: Context) -> Result<Option<Bytes>> {
        self.lookup(key, true, ctx)
            .await
            .map(|(_, content)| Some(content))
    }
}

#[derive(Clone, PartialEq)]
enum Status {
    Initial,
    Responded,
}

#[derive(Clone)]
pub struct LookupNode {
    key: NodePublicKey,
    index: NodeIndex,
    last_responded: Option<u64>,
    status: Status,
}

struct PendingResponse {
    node: LookupNode,
    token: u32,
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
