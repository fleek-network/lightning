use std::cell::OnceCell;
use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};
use std::io;
use std::net::SocketAddr;

use fleek_crypto::NodePublicKey;
use hp_fixed::unsigned::HpUfixed;
use infusion::c;
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::types::NodeIndex;
use lightning_interfaces::{ApplicationInterface, SyncQueryRunnerInterface, TopologyInterface};
use lightning_metrics::histogram;
use lightning_utils::application::QueryRunnerExt;

use crate::event::PoolTask;
use crate::muxer::{ConnectionInterface, MuxerInterface};
use crate::overlay::{
    BroadcastRequest,
    BroadcastTask,
    ConnectionInfo,
    Message,
    Param,
    SendRequest,
};
use crate::state::NodeInfo;

pub type BoxedFilterCallback = Box<dyn Fn(NodeIndex) -> bool + Send + Sync + 'static>;

// Manager (BRAIN)
// Event::NewConnection -> update brain.
// Event::ConnectionEnded -> update brain.
// Event::EpochChanged -> update brain.
// Event::MessageReceived -> validated by Brain and sent to client.
// Event::RequestReceived -> validated by Brain and sent to client.
// Event::BroadcastRequest -> talk to pool (pool will start a connection if needed and send in your
// behalf). Event::SendRequest -> talk to pool (pool will start a connection if needed and send in
// your behalf).
pub struct LogicalPool<C: Collection> {
    /// Peers that we are currently connected to.
    pub(crate) pool: HashMap<NodeIndex, ConnectionInfo>,
    /// Source of pool state.
    topology: c![C::TopologyInterface],
    /// Query Runner.
    sync_query: c![C::ApplicationInterface::SyncExecutor],
    /// Local node index.
    index: OnceCell<NodeIndex>,
    /// Stats.
    stats: Stats,
    /// Local node public key.
    pk: NodePublicKey,
}

impl<C> LogicalPool<C>
where
    C: Collection,
{
    pub fn new(
        sync_query: c!(C::ApplicationInterface::SyncExecutor),
        topology: c!(C::TopologyInterface),
        pk: NodePublicKey,
        index: OnceCell<NodeIndex>,
    ) -> Self {
        Self {
            pool: HashMap::new(),
            sync_query,
            index,
            stats: Stats::default(),
            pk,
            topology,
        }
    }

    pub fn process_received_message(&self, src: &NodeIndex) -> bool {
        self.pool.contains_key(src)
    }

    pub fn process_received_request(&mut self, src: NodeIndex) -> bool {
        if let Some(info) = self.node_info_from_state(&src) {
            self.pin_connection(src, info);
            true
        } else {
            false
        }
    }

    #[inline]
    pub fn contains(&self, peer: &NodeIndex) -> bool {
        self.pool.contains_key(peer)
    }

    #[inline]
    pub fn update_connections(&mut self) -> BroadcastTask {
        let peers = self
            .topology
            .suggest_connections()
            .iter()
            .flatten()
            .filter_map(|pk| self.pubkey_to_index(*pk))
            .filter(|index| *index != self.get_index())
            .collect::<HashSet<_>>();

        // We keep pinned connections.
        let mut peers_to_drop = Vec::new();
        self.pool.retain(|index, info| {
            let is_in_overlay = peers.contains(index);
            if is_in_overlay {
                // We want to update the flag in case it wasn't initially created by a
                // topology update.
                info.from_topology = true
            }

            if is_in_overlay || info.pinned {
                true
            } else {
                peers_to_drop.push(*index);
                false
            }
        });

        // We get information about the peers.
        let peers = peers
            .into_iter()
            // We ignore connections for which we don't have information on state.
            .filter_map(|index| {
                let info = self.node_info_from_state(&index)?;
                let should_connect = info.index < self.get_index();
                Some(ConnectionInfo {
                    from_topology: true,
                    pinned: false,
                    node_info: info,
                    connect: should_connect,
                })
            })
            .collect::<Vec<_>>();

        // We perform a union.
        for info in peers.iter() {
            self.pool
                .entry(info.node_info.index)
                .or_insert(info.clone());
        }

        // Report stats.
        histogram!(
            "pool_neighborhood_size",
            Some("Size of peers (not pinned and from topology) in our neighborhood."),
            peers.len() as f64
        );

        // We would like to measure how many times we are
        // using request-response service with peers in our cluster.
        histogram!(
            "pool_total_req_res_requests",
            Some("Total number of req-res requests"),
            self.stats.total_req_res_requests as f64
        );
        // Reset.
        self.stats.total_req_res_requests = 0;

        histogram!(
            "pool_cluster_hit",
            Some(
                "Number of times that we are using request-response \
                service with peers in our cluster"
            ),
            self.stats.cluster_hit_count as f64
        );
        // Reset.
        self.stats.cluster_hit_count = 0;

        // We tell the pool who to connect to.
        BroadcastTask::Update {
            keep: self.pool.clone(),
            drop: peers_to_drop,
        }
    }

    pub fn get_index(&self) -> NodeIndex {
        if let Some(index) = self.index.get() {
            *index
        } else if let Some(index) = self.sync_query.pubkey_to_index(&self.pk) {
            self.index.set(index).expect("Failed to set index");
            index
        } else {
            u32::MAX
        }
    }

    pub fn node_info_from_state(&self, peer: &NodeIndex) -> Option<NodeInfo> {
        let info = self
            .sync_query
            .get_node_info::<lightning_interfaces::types::NodeInfo>(peer, |n| n)?;

        Some(NodeInfo {
            index: *peer,
            pk: info.public_key,
            socket_address: SocketAddr::from((info.domain, info.ports.pool)),
        })
    }

    pub fn pin_connection(&mut self, peer: NodeIndex, info: NodeInfo) {
        let our_index = self.get_index();
        self.pool
            .entry(peer)
            .and_modify(|info| info.pinned = true)
            .or_insert({
                let should_connect = peer < our_index;
                ConnectionInfo {
                    pinned: true,
                    from_topology: false,
                    node_info: info,
                    connect: should_connect,
                }
            });
    }

    /// Cleans up a pinned connection.
    ///
    /// This method assumes that there are no active transport connections
    /// before calling it because we could potentially erroneously unpin a connection.
    pub fn clean(&mut self, peer: NodeIndex) {
        if let Entry::Occupied(mut entry) = self.pool.entry(peer) {
            let info = entry.get_mut();
            if !info.from_topology && info.pinned {
                entry.remove();
            } else if info.pinned {
                info.pinned = false;
            }
        }
    }

    /// Returns true if the peer has staked the required amount
    /// to be a valid node in the network, and false otherwise.
    #[inline]
    pub fn validate_stake(&self, peer: NodePublicKey) -> bool {
        match self.sync_query.pubkey_to_index(&peer) {
            None => false,
            Some(ref node_idx) => {
                HpUfixed::from(self.sync_query.get_staking_amount())
                    <= self
                        .sync_query
                        .get_node_info::<HpUfixed<18>>(node_idx, |n| n.stake.staked)
                        .unwrap_or(HpUfixed::<18>::zero())
            },
        }
    }

    pub fn _index_from_connection<M: MuxerInterface>(
        &self,
        connection: &M::Connection,
    ) -> Option<NodeIndex> {
        let pk = connection.peer_identity()?;
        self.sync_query.pubkey_to_index(&pk)
    }

    pub fn pubkey_to_index(&self, peer: NodePublicKey) -> Option<NodeIndex> {
        self.sync_query.pubkey_to_index(&peer)
    }

    pub fn _index_to_pubkey(&self, peer: NodeIndex) -> Option<NodePublicKey> {
        self.sync_query.index_to_pubkey(&peer)
    }

    #[inline]
    pub fn record_req_res_request(&mut self, peer: &NodeIndex) {
        self.stats.total_req_res_requests += 1;

        if let Some(info) = self.pool.get(peer) {
            // If it's pinned, it means it's already been accounted for.
            if info.from_topology && !info.pinned {
                self.stats.cluster_hit_count += 1;
            }
        }
    }

    #[inline]
    pub fn connections(&self) -> HashMap<NodeIndex, ConnectionInfo> {
        self.pool.clone()
    }

    #[inline]
    pub fn get_connection_info(&self, peer: &NodeIndex) -> Option<&ConnectionInfo> {
        self.pool.get(peer)
    }

    pub fn process_outgoing_broadcast(
        &self,
        broadcast_request: BroadcastRequest,
    ) -> Option<PoolTask> {
        let peers: Vec<ConnectionInfo> = match broadcast_request.param {
            Param::Filter(filter) => self
                .pool
                .iter()
                .filter(|(index, info)| info.from_topology && filter(**index))
                .map(|(_, info)| info.clone())
                .collect(),
            Param::Index(index) => self
                .pool
                .get(&index)
                .filter(|info| info.from_topology)
                .map(|info| vec![info.clone()])
                .unwrap_or_default(),
        };

        let message = Message {
            service: broadcast_request.service_scope,
            payload: broadcast_request.message.to_vec(),
        };

        if !peers.is_empty() {
            Some(PoolTask::SendMessage { message, peers })
        } else {
            tracing::warn!(
                "no peers to send the message to: no peers in state or filter was too restrictive"
            );
            None
        }
    }

    pub fn process_outgoing_request(&mut self, request: SendRequest) -> Option<PoolTask> {
        match self.node_info_from_state(&request.peer) {
            Some(info) => {
                self.record_req_res_request(&request.peer);
                self.pin_connection(request.peer, info.clone());
                Some(PoolTask::SendRequest {
                    dst: info,
                    service: request.service_scope,
                    request: request.request,
                    respond: request.respond,
                })
            },
            None => {
                if request
                    .respond
                    .send(Err(io::ErrorKind::AddrNotAvailable.into()))
                    .is_err()
                {
                    tracing::error!("requester dropped the channel")
                }
                None
            },
        }
    }
}

/// Overlay stats.
#[derive(Default)]
pub struct Stats {
    total_req_res_requests: usize,
    /// Number of times that we are using request-response
    /// service with peers in our cluster.
    cluster_hit_count: usize,
}
