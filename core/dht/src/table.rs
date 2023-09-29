use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use fleek_crypto::NodePublicKey;
use lightning_interfaces::infu_collection::{c, Collection};
use lightning_interfaces::{ReputationAggregatorInterface, ReputationQueryInteface};
use thiserror::Error;
use tokio::sync::mpsc::{self, Receiver};
use tokio::sync::{oneshot, Notify};

use crate::bucket::{Bucket, BUCKET_REFRESH_INTERVAL, MAX_BUCKETS, MAX_BUCKET_SIZE};
use crate::distance;
use crate::node::NodeInfo;
use crate::task::Task;

const DELTA: u64 = Duration::from_secs(600).as_millis() as u64; // 10 minutes

pub type TableKey = [u8; 32];

pub async fn start_worker<C: Collection>(
    mut rx: Receiver<TableRequest>,
    local_key: NodePublicKey,
    task_tx: mpsc::Sender<Task>,
    local_rep_query: c![C::ReputationAggregatorInterface::ReputationQuery],
    shutdown_notify: Arc<Notify>,
) {
    let mut table = Table::new(local_key);

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
                                    match (a.last_responded, b.last_responded) {
                                        (Some(a_last_res), Some(b_last_res)) => {
                                            let score_a = local_rep_query.get_reputation_of(&a.key);
                                            let score_b = local_rep_query.get_reputation_of(&b.key);
                                            if a_last_res.abs_diff(b_last_res) < DELTA
                                                && (score_a.is_some() || score_b.is_some()) {
                                                // If the dfference between the timestamps of the
                                                // last responses is less than some specified delta,
                                                // we use the local reputation as a tie breaker.
                                                score_a.cmp(&score_b)
                                            } else {
                                                a_last_res.cmp(&b_last_res)
                                            }
                                        },
                                        (Some(_), None) => Ordering::Greater,
                                        (None, Some(_)) => Ordering::Less,
                                        (None, None) => Ordering::Equal,
                                    }
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

pub struct Table {
    local_node_key: NodePublicKey,
    buckets: Vec<Bucket>,
}

impl Table {
    pub fn new(local_node_key: NodePublicKey) -> Self {
        Self {
            local_node_key,
            buckets: vec![Bucket::new()],
        }
    }

    // Returns closest nodes to target in increasing order.
    pub fn closest_nodes(&self, target: &TableKey) -> Vec<NodeInfo> {
        // Todo: Filter good vs bad nodes based on some criteria.
        let mut closest = BTreeMap::new();
        let distance = distance::distance(&self.local_node_key.0, target);
        // We use this set to avoid traversing a bucket more than once since
        // some bits in the XORed value could be mapped to the same bucket.
        let mut visited = HashSet::new();
        let mut zero_bit_indexes = Vec::new();
        // First, visit every bucket, such that its corresponding bit in the XORed value is 1,
        // in decreasing order from MSB.
        for (count, byte) in distance.iter().enumerate() {
            let mask = 128u8;
            for shift in 0..8u8 {
                let possible_index = count * 8 + shift as usize;
                let index = calculate_bucket_index(self.buckets.len(), possible_index);
                if visited.contains(&index) {
                    continue;
                }
                if (byte & (mask >> shift)) > 0 {
                    for node in self.buckets[index].nodes() {
                        let distance = distance::distance(target, &node.key.0);
                        closest.insert(distance, node.clone());
                        if closest.len() >= MAX_BUCKET_SIZE {
                            return closest.into_values().collect();
                        }
                    }
                } else {
                    zero_bit_indexes.push(index)
                }
                visited.insert(index);
            }
        }

        // Second, visit every bucket, such that its corresponding bit in the XORed value is 0,
        // in increasing order from LSB.
        for index in zero_bit_indexes.iter().rev() {
            for node in self.buckets[*index].nodes() {
                let distance = distance::distance(target, &node.key.0);
                closest.insert(distance, node.clone());
                if closest.len() >= MAX_BUCKET_SIZE {
                    return closest.into_values().collect();
                }
            }
        }
        closest.into_values().collect()
    }

    fn get_bucket_nodes(&self, bucket_index: usize) -> Option<Vec<NodeInfo>> {
        if bucket_index < self.buckets.len() {
            Some(self.buckets[bucket_index].nodes().cloned().collect())
        } else {
            None
        }
    }

    fn set_bucket_nodes(&mut self, bucket_index: usize, nodes: Vec<NodeInfo>) {
        if bucket_index < self.buckets.len() {
            self.buckets[bucket_index].set_nodes(nodes);
        }
    }

    fn add_node(&mut self, node: NodeInfo) -> Result<()> {
        if node.key == self.local_node_key {
            // We don't add ourselves to the routing table.
            return Ok(());
        }
        self._add_node(node);
        Ok(())
    }

    fn _add_node(&mut self, node: NodeInfo) {
        // Get index of bucket.
        let index = distance::leading_zero_bits(&self.local_node_key.0, &node.key.0);
        assert_ne!(index, MAX_BUCKETS);
        let bucket_index = calculate_bucket_index(self.buckets.len(), index);
        if !self.buckets[bucket_index].add_node(node.clone()) && self.split_bucket(bucket_index) {
            self._add_node(node)
        }
    }

    fn update_node_timestamp(&mut self, node_key: NodePublicKey, timestamp: u64) {
        let index = distance::leading_zero_bits(&self.local_node_key.0, &node_key.0);
        let bucket_index = calculate_bucket_index(self.buckets.len(), index);
        for node in self.buckets[bucket_index].nodes_mut() {
            if node.key == node_key {
                node.last_responded = Some(timestamp);
            }
        }
    }

    fn split_bucket(&mut self, index: usize) -> bool {
        // We split the bucket only if it is the last one.
        // This is because the closest nodes to us will be
        // in the buckets further up in the list.
        // In addition, bucket size decrements.
        if index == MAX_BUCKETS - 1 || index != self.buckets.len() - 1 {
            return false;
        }
        let bucket = self.buckets.pop().expect("there to be at least one bucket");
        self.buckets.push(Bucket::new());
        self.buckets.push(Bucket::new());

        for node in bucket.into_nodes() {
            self._add_node(node)
        }
        true
    }
}

fn calculate_bucket_index(bucket_count: usize, possible_index: usize) -> usize {
    if possible_index >= bucket_count {
        bucket_count - 1
    } else {
        possible_index
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::time::Duration;

    use lightning_application::app::Application;
    use lightning_application::config::{Mode, StorageConfig};
    use lightning_interfaces::{
        partial,
        ApplicationInterface,
        ReputationAggregatorInterface,
        SignerInterface,
    };
    use lightning_rep_collector::ReputationAggregator;
    use lightning_signer::Signer;
    use lightning_topology::Topology;
    use rand::Rng;
    use tokio::sync::mpsc;

    use super::*;
    use crate::bucket;

    partial!(PartialBinding {
        ApplicationInterface = Application<Self>;
        TopologyInterface = Topology<Self>;
        ReputationAggregatorInterface = ReputationAggregator<Self>;
    });

    fn get_random_key() -> TableKey {
        let mut rng = rand::thread_rng();
        let mut array = [0; 32];
        for byte in array.iter_mut() {
            *byte = rng.gen_range(0..255);
        }
        array
    }

    fn get_reputation_query() -> lightning_rep_collector::MyReputationQuery {
        let application = Application::<PartialBinding>::init(
            lightning_application::config::Config {
                mode: Mode::Test,
                genesis: None,
                testnet: false,
                storage: StorageConfig::InMemory,
                db_path: None,
                db_options: None,
            },
            Default::default(),
        )
        .unwrap();
        let query_runner = application.sync_query();

        let signer = Signer::<PartialBinding>::init(
            lightning_signer::Config {
                node_key_path: PathBuf::from("../test-utils/keys/test_node2.pem")
                    .try_into()
                    .expect("Failed to resolve path"),
                consensus_key_path: PathBuf::from("../test-utils/keys/test_consensus2.pem")
                    .try_into()
                    .expect("Failed to resolve path"),
            },
            query_runner.clone(),
        )
        .unwrap();

        let rep_aggregator = ReputationAggregator::<PartialBinding>::init(
            lightning_rep_collector::config::Config::default(),
            signer.get_socket(),
            Default::default(),
            query_runner,
        )
        .unwrap();
        rep_aggregator.get_query()
    }

    #[test]
    fn test_build_table() {
        let my_key = [0; 32];
        let mut table = Table::new(NodePublicKey(my_key));
        let num_nodes = 20;

        let mut set = HashSet::new();
        for i in 0..num_nodes {
            let k = num_nodes - i;
            let mut leading_zeros = 0;
            let mut and_mask = Vec::new();
            let mut or_mask = Vec::new();
            while leading_zeros < k {
                if leading_zeros + 8 <= k {
                    and_mask.push(0u8);
                    or_mask.push(0u8);
                    leading_zeros += 8;
                } else {
                    let byte = 255_u8 >> (k - leading_zeros);
                    and_mask.push(byte);
                    let byte = 128_u8 >> (k - leading_zeros);
                    or_mask.push(byte);
                    leading_zeros += k - leading_zeros;
                }
            }
            while and_mask.len() < 32 {
                and_mask.push(255_u8);
                or_mask.push(0_u8);
            }

            let key = loop {
                let mut key = get_random_key();
                for j in 0..key.len() {
                    key[j] &= and_mask[j];
                    key[j] |= or_mask[j];
                }
                if !set.contains(&key) {
                    break key;
                }
            };
            set.insert(key);

            let node = NodeInfo {
                address: "0.0.0.0:0".parse().unwrap(),
                key: key.into(),
                last_responded: None,
            };
            table.add_node(node).unwrap();
        }

        let mut actual_count = 0;
        for bucket in table.buckets.iter() {
            actual_count += bucket.nodes().count();
        }
        assert_eq!(actual_count, num_nodes);
    }

    #[tokio::test]
    async fn test_propose_nodes() {
        let public_key = get_random_key();
        let (table_tx, table_rx) = mpsc::channel(10);
        let shutdown_notify = Arc::new(tokio::sync::Notify::new());

        let (tx, _) = mpsc::channel(10);
        let worker_fut = start_worker::<PartialBinding>(
            table_rx,
            public_key.into(),
            tx,
            //rep_aggregator.get_query(),
            get_reputation_query(),
            shutdown_notify.clone(),
        );

        let request_fut = async move {
            tokio::time::sleep(Duration::from_secs(2)).await;
            let node1 = NodeInfo {
                address: "127.0.0.1:8000".parse().unwrap(),
                key: bucket::random_key_in_bucket(0, &public_key).into(),
                last_responded: Some(100),
            };
            let node2 = NodeInfo {
                address: "127.0.0.1:8001".parse().unwrap(),
                key: bucket::random_key_in_bucket(0, &public_key).into(),
                last_responded: Some(200),
            };
            let node3 = NodeInfo {
                address: "127.0.0.1:8002".parse().unwrap(),
                key: bucket::random_key_in_bucket(0, &public_key).into(),
                last_responded: None,
            };
            let req = TableRequest::ProposeBucketNodes {
                bucket_index: 0,
                nodes: vec![node1.clone(), node2.clone(), node3],
            };
            table_tx.send(req).await.unwrap();

            let node4 = NodeInfo {
                address: "127.0.0.1:8003".parse().unwrap(),
                key: bucket::random_key_in_bucket(0, &public_key).into(),
                last_responded: Some(500),
            };
            let node5 = NodeInfo {
                address: "127.0.0.1:8004".parse().unwrap(),
                key: bucket::random_key_in_bucket(0, &public_key).into(),
                last_responded: None,
            };
            let node6 = NodeInfo {
                address: "127.0.0.1:8005".parse().unwrap(),
                key: bucket::random_key_in_bucket(0, &public_key).into(),
                last_responded: Some(50),
            };
            let req = TableRequest::ProposeBucketNodes {
                bucket_index: 0,
                nodes: vec![node4.clone(), node5, node6],
            };
            table_tx.send(req).await.unwrap();

            let (tx, rx) = oneshot::channel();
            let req = TableRequest::GetBucketNodes {
                bucket_index: 0,
                respond: tx,
            };
            table_tx.send(req).await.unwrap();

            let nodes = rx.await.unwrap();
            let nodes: HashMap<NodePublicKey, NodeInfo> = nodes
                .unwrap()
                .into_iter()
                .map(|node| (node.key, node))
                .collect();

            assert_eq!(nodes.len(), 3);
            assert!(nodes.contains_key(&node1.key));
            assert!(nodes.contains_key(&node2.key));
            assert!(nodes.contains_key(&node4.key));

            shutdown_notify.notify_one();
        };

        tokio::select! {
            _ = worker_fut => {}
            _ = request_fut => {}
        }
    }

    #[tokio::test]
    async fn test_propose_nodes_different_indices() {
        let public_key = get_random_key();
        let (table_tx, table_rx) = mpsc::channel(10);
        let shutdown_notify = Arc::new(tokio::sync::Notify::new());

        let (tx, _) = mpsc::channel(10);
        let worker_fut = start_worker::<PartialBinding>(
            table_rx,
            public_key.into(),
            tx,
            get_reputation_query(),
            shutdown_notify.clone(),
        );

        let request_fut = async move {
            tokio::time::sleep(Duration::from_secs(2)).await;
            let node1 = NodeInfo {
                address: "127.0.0.1:8000".parse().unwrap(),
                key: bucket::random_key_in_bucket(0, &public_key).into(),
                last_responded: Some(100),
            };
            let node2 = NodeInfo {
                address: "127.0.0.1:8001".parse().unwrap(),
                key: bucket::random_key_in_bucket(1, &public_key).into(),
                last_responded: Some(200),
            };
            let node3 = NodeInfo {
                address: "127.0.0.1:8002".parse().unwrap(),
                key: bucket::random_key_in_bucket(2, &public_key).into(),
                last_responded: None,
            };
            let req = TableRequest::ProposeBucketNodes {
                bucket_index: 0,
                nodes: vec![node1, node2.clone(), node3],
            };
            table_tx.send(req).await.unwrap();

            let node4 = NodeInfo {
                address: "127.0.0.1:8003".parse().unwrap(),
                key: bucket::random_key_in_bucket(0, &public_key).into(),
                last_responded: Some(500),
            };
            let node5 = NodeInfo {
                address: "127.0.0.1:8004".parse().unwrap(),
                key: bucket::random_key_in_bucket(1, &public_key).into(),
                last_responded: None,
            };
            let node6 = NodeInfo {
                address: "127.0.0.1:8005".parse().unwrap(),
                key: bucket::random_key_in_bucket(2, &public_key).into(),
                last_responded: Some(50),
            };
            let req = TableRequest::ProposeBucketNodes {
                bucket_index: 0,
                nodes: vec![node4.clone(), node5, node6.clone()],
            };
            table_tx.send(req).await.unwrap();

            let (tx, rx) = oneshot::channel();
            let req = TableRequest::GetBucketNodes {
                bucket_index: 0,
                respond: tx,
            };
            table_tx.send(req).await.unwrap();

            let nodes = rx.await.unwrap();
            let nodes: HashMap<NodePublicKey, NodeInfo> = nodes
                .unwrap()
                .into_iter()
                .map(|node| (node.key, node))
                .collect();

            assert_eq!(nodes.len(), 3);
            assert!(nodes.contains_key(&node6.key));
            assert!(nodes.contains_key(&node2.key));
            assert!(nodes.contains_key(&node4.key));

            shutdown_notify.notify_one();
        };

        tokio::select! {
            _ = worker_fut => {}
            _ = request_fut => {}
        }
    }
}
