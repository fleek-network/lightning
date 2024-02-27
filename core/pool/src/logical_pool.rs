use std::cell::OnceCell;
use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};
use std::io;
use std::net::SocketAddr;
use std::sync::Arc;

use bytes::Bytes;
use fleek_crypto::NodePublicKey;
use infusion::c;
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::types::NodeIndex;
use lightning_interfaces::{ApplicationInterface, ServiceScope, SyncQueryRunnerInterface};
use lightning_metrics::histogram;
use serde::{Deserialize, Serialize};
use tokio::sync::oneshot;

use crate::endpoint::EndpointTask;
use crate::event::{Message, Param};
use crate::provider::Response;
use crate::state::NodeInfo;

/// Pool that handles logical connections.
pub struct LogicalPool<C: Collection> {
    /// Peers that we are currently connected to.
    pub(crate) pool: HashMap<NodeIndex, ConnectionInfo>,
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
    pub fn new(sync_query: c!(C::ApplicationInterface::SyncExecutor), pk: NodePublicKey) -> Self {
        Self {
            pool: HashMap::new(),
            sync_query,
            index: OnceCell::new(),
            stats: Stats::default(),
            pk,
        }
    }

    #[inline]
    pub fn clear_state(&mut self) {
        self.pool.clear();
    }

    #[inline]
    pub fn handle_new_connection(&mut self, peer: NodeIndex, _: bool) {
        if !self.contains(&peer) {
            if let Some(info) = self.node_info_from_state(&peer) {
                self.pin_connection(peer, info);
            }
        }
    }

    #[inline]
    pub fn process_received_message(&self, src: &NodeIndex) -> bool {
        self.pool.contains_key(src)
    }

    #[inline]
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

    // This is for unit tests.
    pub(crate) fn _update_connections(&mut self, peers: HashSet<NodeIndex>) -> EndpointTask {
        // We keep pinned connections.
        let mut peers_to_drop = Vec::new();
        self.pool.retain(|index, info| {
            let is_in_new_cluster = peers.contains(index);
            if is_in_new_cluster {
                // We want to update the flag in case it wasn't initially created by a
                // topology update.
                info.from_topology = true
            }

            if is_in_new_cluster || info.pinned {
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
        EndpointTask::Update {
            keep: self.pool.clone(),
            drop: peers_to_drop,
        }
    }

    pub fn update_connections(
        &mut self,
        connections: Arc<Vec<Vec<NodePublicKey>>>,
    ) -> EndpointTask {
        let peers = connections
            .iter()
            .flatten()
            .filter_map(|pk| self.pubkey_to_index(*pk))
            .filter(|index| *index != self.get_index())
            .collect::<HashSet<_>>();

        self._update_connections(peers)
    }

    #[inline]
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

    #[inline]
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

    #[inline]
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

    #[inline]
    pub fn pubkey_to_index(&self, peer: NodePublicKey) -> Option<NodeIndex> {
        self.sync_query.pubkey_to_index(&peer)
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
    pub fn _get_connection_info(&self, peer: &NodeIndex) -> Option<&ConnectionInfo> {
        self.pool.get(peer)
    }

    pub fn process_outgoing_broadcast(
        &self,
        service_scope: ServiceScope,
        message: Bytes,
        param: Param,
    ) -> Option<EndpointTask> {
        let peers: Vec<ConnectionInfo> = match param {
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
            service: service_scope,
            payload: message.to_vec(),
        };

        if !peers.is_empty() {
            Some(EndpointTask::SendMessage { message, peers })
        } else {
            tracing::warn!(
                "no peers to send the message to: no peers in state or filter was too restrictive"
            );
            None
        }
    }

    pub fn process_outgoing_request(
        &mut self,
        dst: NodeIndex,
        service_scope: ServiceScope,
        request: Bytes,
        respond: oneshot::Sender<io::Result<Response>>,
    ) -> Option<EndpointTask> {
        match self.node_info_from_state(&dst) {
            Some(info) => {
                self.record_req_res_request(&dst);
                self.pin_connection(dst, info.clone());
                Some(EndpointTask::SendRequest {
                    dst: info,
                    service: service_scope,
                    request,
                    respond,
                })
            },
            None => {
                if respond
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

/// Logical pool stats.
#[derive(Default)]
pub struct Stats {
    total_req_res_requests: usize,
    /// Number of times that we are using request-response
    /// service with peers in our cluster.
    cluster_hit_count: usize,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ConnectionInfo {
    /// Pinned connections should not be dropped
    /// on topology changes.
    pub pinned: bool,
    /// This connection was initiated on a topology event.
    pub from_topology: bool,
    /// The info of the peer.
    pub node_info: NodeInfo,
    /// This field is used during topology updates.
    /// It tells the pool whether it should connect
    /// to the peer or wait for the peer to connect.
    pub connect: bool,
}
