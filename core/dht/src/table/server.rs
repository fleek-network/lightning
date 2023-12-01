use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use anyhow::Result;
use bytes::Bytes;
use fleek_crypto::NodePublicKey;
use futures::stream::FuturesUnordered;
use lightning_interfaces::infu_collection::{c, Collection};
use lightning_interfaces::types::NodeIndex;
use lightning_interfaces::{
    ApplicationInterface,
    ReputationAggregatorInterface,
    SyncQueryRunnerInterface,
};
use thiserror::Error;
use tokio::sync::mpsc::{self, Receiver};
use tokio::sync::{oneshot, Notify};
use tokio::task::JoinHandle;

use crate::node::NodeInfo;
use crate::pool;
use crate::pool::ValueRespond;
use crate::table::bucket::{BUCKET_REFRESH_INTERVAL, MAX_BUCKETS, MAX_BUCKET_SIZE};
use crate::table::manager::{Manager, StdManager};
use crate::table::{bucket, distance, Event};
use crate::task::Task;

pub type TableKey = [u8; 32];

pub struct Server<C: Collection, M> {
    /// Our node index.
    us: TableKey,
    /// Queue on incoming requests.
    request_queue: Receiver<Request>,
    /// Queue on incoming events.
    event_queue: Receiver<Event>,
    /// Local store.
    store: HashMap<TableKey, Bytes>,
    /// Table manager.
    manager: M,
    /// Pool client.
    pool: pool::Client,
    /// Sync query.
    sync_query: c![C::ApplicationInterface::SyncExecutor],
    /// Bootstrap-related tasks.
    bootstrap_tasks: FuturesUnordered<JoinHandle<Result<Vec<NodeIndex>>>>,
    /// State of bootstrap process.
    bootstrap_state: BootstrapState,
    /// Shutdown notify.
    shutdown: Arc<Notify>,
}

impl<C, M> Server<C, M>
where
    C: Collection,
    M: Manager,
{
    pub fn handle_request(&mut self, request: Request) {
        match request {
            Request::Get {
                key,
                local,
                respond,
            } => {
                if local {
                    self.local_get(&key, respond);
                } else {
                    self.get(key, respond);
                }
            },
            Request::Put { key, value, local } => {
                if local {
                    self.local_put(key, value);
                } else {
                    self.put(key, value);
                }
            },
            Request::ClosestContacts { key, respond } => {
                let _ = respond.send(self.manager.closest_contacts(key));
            },
            Request::Bootstrap { bootstrap_nodes } => {},
        }
    }

    fn bootstrap(&mut self, bootstrap_nodes: Vec<NodeIndex>) {
        if matches!(self.bootstrap_state, BootstrapState::Idle) {
            self.add_contacts(bootstrap_nodes);

            // Todo: Handle error.
            // Do self lookup.
            let (lookup_result_tx, lookup_result_rx) = oneshot::channel();
            let _ = self.pool.lookup_contact(self.us, lookup_result_tx);

            self.bootstrap_tasks
                .push(tokio::spawn(async move { lookup_result_rx.await? }));

            self.bootstrap_state = BootstrapState::Initial;
        }
    }

    fn handle_bootstrap_event(&mut self, contacts: Vec<NodeIndex>) {
        match self.bootstrap_state {
            BootstrapState::Initial => {
                // Add new contacts to the table.
                self.add_contacts(contacts);

                // Here we want to find the first non-empty bucket (logically to us)
                // and generate random keys for every bucket starting from
                // the aforementioned bucket.
                // We start a lookup task for each of those keys.
                let closest = self.manager.closest_contact_info(self.us);
                match closest.first() {
                    Some(node) => {
                        let index = distance::leading_zero_bits(&node.key.0, &self.us);

                        let random_keys = (index..MAX_BUCKETS)
                            .map(|index| bucket::random_key_in_bucket(index, &self.us))
                            .collect::<HashSet<_>>();

                        for next_key in random_keys {
                            let (lookup_result_tx, lookup_result_rx) = oneshot::channel();
                            let _ = self.pool.lookup_contact(next_key, lookup_result_tx);

                            self.bootstrap_tasks
                                .push(tokio::spawn(async move { lookup_result_rx.await? }));
                        }

                        self.bootstrap_state = BootstrapState::Looking;
                    },
                    None => {
                        tracing::error!("failed to find closest nodes");
                        // Nothing left to do.
                        self.bootstrap_state = BootstrapState::Idle;
                    },
                }
            },
            BootstrapState::Looking => {
                self.add_contacts(contacts);
            },
            BootstrapState::Idle => {
                unreachable!("invalid bootstrapping state");
            },
        }
    }

    fn add_contacts(&mut self, contacts: Vec<NodeIndex>) {
        for index in contacts {
            if let Some(key) = self.sync_query.index_to_pubkey(index) {
                let _ = self.manager.add_node(NodeInfo {
                    // Todo: Remove this field.
                    address: "0.0.0.0:0".parse().unwrap(),
                    index,
                    key,
                    last_responded: None,
                });
            }
        }
    }

    fn put(&mut self, key: TableKey, value: Bytes) {
        if let Err(e) = self.pool.store(key, value) {
            tracing::error!("`put` failed: {e:?}");
        }
    }

    fn local_put(&mut self, key: TableKey, value: Bytes) {
        self.store.insert(key, value);
    }

    fn get(&self, key: TableKey, respond: ValueRespond) {
        if self.store.contains_key(&key) {
            self.local_get(&key, respond)
        } else {
            if let Err(e) = self.pool.lookup_value(key, respond) {
                tracing::error!("`get` failed: {e:?}");
            }
        }
    }

    fn local_get(&self, key: &TableKey, respond: ValueRespond) {
        let entry = self.store.get(key).cloned();
        let _ = respond.send(Ok(entry));
    }

    pub async fn run(&mut self) -> Result<()> {
        loop {
            tokio::select! {
                _ = self.shutdown.notified() => {
                    break;
                }
                next = self.request_queue.recv() => {
                    let Some(request) = next else {
                        break;
                    };
                    self.handle_request(request);
                }
                next = self.event_queue.recv() => {
                    let Some(event) = next else {
                        break
                    };
                    self.manager.handle_event(event);
                }
            }
        }

        Ok(())
    }
}

pub async fn start_worker<C: Collection>(
    mut rx: Receiver<TableRequest>,
    local_key: NodePublicKey,
    task_tx: mpsc::Sender<Task>,
    _local_rep_query: c![C::ReputationAggregatorInterface::ReputationQuery],
    shutdown_notify: Arc<Notify>,
) {
    let mut table = StdManager::new(local_key);

    // We always start with one bucket, so kick off the refresh task for this bucket.
    let task = Task::RefreshBucket {
        bucket_index: table.buckets.len() - 1,
        delay: BUCKET_REFRESH_INTERVAL,
    };
    if let Err(e) = task_tx.send(task).await {
        tracing::trace!("failed to send bucket refresh task: {e:?}");
    }
    loop {
        tokio::select! {
            request = rx.recv() => {
                if let Some(request) = request {
                    match request {
                        TableRequest::ClosestNodes { target: key, respond } => {
                            let nodes = table.closest_nodes(&key);
                            respond.send(Ok(nodes))
                                .expect("internal table client not to drop the channel");
                        },
                        TableRequest::AddNode { node, respond } => {
                            let num_buckets = table.buckets.len();
                            let result = table
                                .add_node(node)
                                .map_err(|e| QueryError(e.to_string()));

                            if num_buckets < table.buckets.len() {
                                // A new bucket was created.
                                let task = Task::RefreshBucket {
                                    bucket_index: table.buckets.len() - 1,
                                    delay: BUCKET_REFRESH_INTERVAL,
                                };
                                if let Err(e) = task_tx.send(task).await {
                                    tracing::trace!("failed to send bucket refresh task: {e:?}");
                                }
                            }
                            if let Some(respond) = respond {
                                respond.send(result)
                                    .expect("internal table client not to drop the channel");
                            }
                        },
                        TableRequest::NearestNeighborsBucket { respond } => {
                            let local_key = table.local_node_key;
                            let closest = table.closest_nodes(&local_key.0);
                            match &closest.first() {
                                Some(node) => {
                                    let index = distance::leading_zero_bits(
                                        &node.key.0,
                                        &local_key.0
                                    );
                                    respond.send(Some(index))
                                        .expect("internal table client not to drop the channel");
                                },
                                None => {
                                    respond.send(None)
                                        .expect("internal table client not to drop the channel");
                                },
                            }
                        },
                        TableRequest::UpdateNodeTimestamp { node_key, timestamp } => {
                            table.update_node_timestamp(node_key, timestamp);
                        }
                        TableRequest::ProposeBucketNodes { bucket_index, nodes } => {
                            let mut actual_buckets = HashMap::new();
                            for node in nodes {
                                let index = distance::leading_zero_bits(&local_key.0, &node.key.0);
                                actual_buckets.entry(index).or_insert(Vec::new()).push(node);
                            }
                            if let Some(nodes) = table.get_bucket_nodes(bucket_index) {
                                for node in nodes {
                                    let index = distance::leading_zero_bits(
                                        &local_key.0,
                                        &node.key.0
                                    );
                                    actual_buckets.entry(index).or_insert(Vec::new()).push(node);
                                }
                            }
                            for bucket in actual_buckets.values_mut() {
                                bucket.sort_by(|a, b| {
                                    // Sort the bucket such that the most desirable nodes are at the end of
                                    // the vec.
                                    //match (a.last_responded, b.last_responded) {
                                    //    (Some(a_last_res), Some(b_last_res)) => {
                                    //        let score_a = local_rep_query.get_reputation_of(&a.key);
                                    //        let score_b = local_rep_query.get_reputation_of(&b.key);
                                    //        if a_last_res.abs_diff(b_last_res) < DELTA
                                    //            && (score_a.is_some() || score_b.is_some()) {
                                    //            // If the dfference between the timestamps of the
                                    //            // last responses is less than some specified delta,
                                    //            // we use the local reputation as a tie breaker.
                                    //            score_a.cmp(&score_b)
                                    //        } else {
                                    //            a_last_res.cmp(&b_last_res)
                                    //        }
                                    //    },
                                    //    (Some(_), None) => Ordering::Greater,
                                    //    (None, Some(_)) => Ordering::Less,
                                    //    (None, None) => Ordering::Equal,
                                    //}
                                    a.last_responded.cmp(&b.last_responded)
                                });
                            }
                            // Pick the most desirable nodes from each bucket.
                            // Make sure that we pick approximately the same amount of nodes from
                            // each bucket.
                            let mut fresh_nodes = Vec::new();
                            'outer: loop {
                                let mut all_empty = true;
                                for nodes in actual_buckets.values_mut() {
                                    if let Some(node) = nodes.pop() {
                                        fresh_nodes.push(node);
                                    }
                                    if !nodes.is_empty() {
                                        all_empty = false;
                                    }
                                    if fresh_nodes.len() == MAX_BUCKET_SIZE {
                                        break 'outer;
                                    }
                                }
                                if all_empty {
                                    break;
                                }
                            }
                            table.set_bucket_nodes(bucket_index, fresh_nodes);
                        }
                        #[cfg(test)]
                        TableRequest::GetBucketNodes { bucket_index, respond } => {
                            respond.send(table.get_bucket_nodes(bucket_index)).unwrap();
                        }
                    }
                }
            }
            _ = shutdown_notify.notified() => {
                tracing::info!("shutting down table worker");
                break;
            }
        }
    }
}

#[derive(Debug, Error)]
#[error("querying the table failed: {0}")]
pub struct QueryError(String);

pub enum TableRequest {
    ClosestNodes {
        target: TableKey,
        respond: oneshot::Sender<Result<Vec<NodeInfo>, QueryError>>,
    },
    AddNode {
        node: NodeInfo,
        respond: Option<oneshot::Sender<Result<(), QueryError>>>,
    },
    // Returns index for non-empty bucket containing closest neighbors. Used for bootstrapping.
    NearestNeighborsBucket {
        respond: oneshot::Sender<Option<usize>>,
    },
    UpdateNodeTimestamp {
        node_key: NodePublicKey,
        timestamp: u64,
    },
    #[allow(unused)]
    ProposeBucketNodes {
        bucket_index: usize,
        nodes: Vec<NodeInfo>,
    },
    #[cfg(test)]
    GetBucketNodes {
        bucket_index: usize,
        respond: oneshot::Sender<Option<Vec<NodeInfo>>>,
    },
}

pub enum Request {
    Get {
        key: TableKey,
        respond: ValueRespond,
        // Whether GET operation will be applied to local storage or DHT.
        local: bool,
    },
    Put {
        key: TableKey,
        value: Bytes,
        // Whether PUT operation will be applied to local storage or DHT.
        local: bool,
    },
    ClosestContacts {
        key: TableKey,
        respond: oneshot::Sender<Vec<NodeIndex>>,
    },
    Bootstrap {
        bootstrap_nodes: Vec<NodeIndex>,
    },
}

enum BootstrapState {
    Idle,
    Initial,
    Looking,
}
