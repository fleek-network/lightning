use std::collections::VecDeque;
use std::marker::PhantomData;
use std::time::{Duration, Instant};

use fxhash::{FxHashMap, FxHashSet};
use lightning_interfaces::schema::broadcast::MessageInternedId;
use lightning_interfaces::types::NodeIndex;
use rand::distributions::WeightedIndex;
use rand::prelude::Distribution;
use rand::thread_rng;
use ta::indicators::ExponentialMovingAverage;
use ta::Next;

use crate::stats::FusedTa;
use crate::BroadcastBackend;

const TICK_DURATION: Duration = Duration::from_millis(500);
const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_millis(1000);
const BUFFER_SIZE: usize = 1000;

/// Responsible for keeping track of the pending want requests we have
/// out there.
///
/// The desired functionality is that we need to store an outgoing
/// want request here.
pub struct PendingStore<B: BroadcastBackend> {
    /// Map each pending digest to a set of nodes that we know have this digest.
    pending_map: FxHashMap<MessageInternedId, FxHashSet<NodeIndex>>,

    /// Map each pending digest to a list of requests we sent out.
    request_map: FxHashMap<MessageInternedId, VecDeque<PendingRequest>>,

    /// RTTs for the peers we interacted with.
    rtt_map: FxHashMap<NodeIndex, FusedTa<f64, ExponentialMovingAverage>>,

    _marker: PhantomData<B>,
}

impl<B: BroadcastBackend> PendingStore<B> {
    pub fn new() -> Self {
        Self {
            pending_map: FxHashMap::default(),
            request_map: FxHashMap::default(),
            rtt_map: FxHashMap::default(),
            _marker: PhantomData,
        }
    }

    pub fn insert_pending(&mut self, node: NodeIndex, interned_id: MessageInternedId) {
        self.pending_map
            .entry(interned_id)
            .or_default()
            .insert(node);
    }

    pub fn insert_request(&mut self, node: NodeIndex, interned_id: MessageInternedId) {
        let requests = self.request_map.entry(interned_id).or_default();
        if requests.len() >= BUFFER_SIZE {
            requests.pop_front();
        }
        requests.push_back(PendingRequest::new(node));
    }

    pub async fn tick(&mut self) -> Vec<(MessageInternedId, NodeIndex)> {
        loop {
            let pending_requests = self.get_pending_requests();
            let pending_requests = self.select_request_receivers(pending_requests);
            // Mark the messages as requested if there are any.
            pending_requests.iter().for_each(|(id, pub_key)| {
                self.insert_request(*pub_key, *id);
                // Remove node from pending map to avoid sending the same request twice to the same
                // node for the same digest.
                if let Some(node_set) = self.pending_map.get_mut(id) {
                    node_set.remove(pub_key);
                }
            });
            if !pending_requests.is_empty() {
                return pending_requests;
            }
            tokio::time::sleep(TICK_DURATION).await;
        }
    }

    pub fn received_message(&mut self, node: NodeIndex, interned_id: MessageInternedId) {
        self.request_map
            .get(&interned_id)
            .map(|requests| requests.iter().find(|req| req.node == node))
            .map(|req| {
                req.map(|req| {
                    let rtt = req.timestamp.elapsed().as_millis() as f64;
                    self.rtt_map
                        .entry(node)
                        .or_insert(FusedTa::from(ExponentialMovingAverage::default()))
                        .next(rtt);
                })
            });
    }

    pub fn remove_message(&mut self, interned_id: MessageInternedId) {
        let _ = self.pending_map.remove(&interned_id);
        let _ = self.request_map.remove(&interned_id);
    }

    fn select_request_receivers(
        &self,
        requests: Vec<(MessageInternedId, FxHashSet<NodeIndex>)>,
    ) -> Vec<(MessageInternedId, NodeIndex)> {
        requests
            .into_iter()
            .filter_map(|(id, pub_keys)| {
                let pub_keys: Vec<NodeIndex> = pub_keys.iter().copied().collect();
                if pub_keys.is_empty() {
                    None
                } else if pub_keys.len() == 1 {
                    Some((id, pub_keys[0]))
                } else {
                    let mut max_val: f64 = 0.0;
                    let weights: Vec<f64> = pub_keys
                        .iter()
                        .map(|pub_key| self.rtt_map.get(pub_key).and_then(|ema| ema.current()))
                        .map(|ema| {
                            let ema = ema.unwrap_or(0.0);
                            max_val = max_val.max(ema);
                            ema
                        })
                        .collect();
                    // Randomly sample the node we will send the request to for a particular
                    // message.
                    // The sampling probability for a particular node is inversely proportional
                    // to the measured RTT (the lower the RTT, the higher the chance of
                    // selecting the node).
                    let weights: Vec<f64> =
                        weights.into_iter().map(|w| max_val - w + 1.0).collect();
                    let peer = if let Ok(dist) = WeightedIndex::new(weights) {
                        let mut rng = thread_rng();
                        pub_keys[dist.sample(&mut rng)]
                    } else {
                        pub_keys[0]
                    };
                    Some((id, peer))
                }
            })
            .collect()
    }

    fn get_pending_requests(&self) -> Vec<(MessageInternedId, FxHashSet<NodeIndex>)> {
        self.pending_map
            .iter()
            .filter(|(id, _pub_keys)| {
                if let Some(requests) = self.request_map.get(id) {
                    if let Some(req) = requests.back() {
                        // If the most recent request for this message is too far in the past,
                        // we send another request.
                        // We use the EMA of the RTT for a particular node to determine the
                        // timeout.
                        let timeout = self
                            .rtt_map
                            .get(&req.node)
                            .and_then(|ema| ema.current())
                            .map(|ema| ema * 2.0)
                            .unwrap_or(DEFAULT_REQUEST_TIMEOUT.as_millis() as f64)
                            as u128;
                        req.timestamp.elapsed().as_millis() >= timeout
                    } else {
                        // We haven't send a request for this digest.
                        true
                    }
                } else {
                    // We haven't send a request for this digest.
                    true
                }
            })
            .map(|(id, pub_keys)| (*id, pub_keys.clone()))
            .collect()
    }
}

struct PendingRequest {
    /// The node we sent the want request to
    node: NodeIndex,
    /// Timestamp when we sent out the request.
    timestamp: Instant,
}

impl PendingRequest {
    pub fn new(node: NodeIndex) -> Self {
        Self {
            node,
            timestamp: Instant::now(),
        }
    }
}
