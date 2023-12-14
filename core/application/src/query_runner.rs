use std::collections::{BTreeSet, HashMap};
use std::time::Duration;

use atomo::{Atomo, QueryPerm, ResolvedTableReference};
use autometrics::autometrics;
use fleek_crypto::{ClientPublicKey, EthAddress, NodePublicKey};
use hp_fixed::unsigned::HpUfixed;
use lightning_interfaces::application::SyncQueryRunnerInterface;
use lightning_interfaces::types::{
    AccountInfo,
    Blake3Hash,
    Committee,
    CommodityTypes,
    Epoch,
    EpochInfo,
    Metadata,
    NodeIndex,
    NodeInfo,
    NodeInfoWithIndex,
    NodeServed,
    ProtocolParams,
    ReportedReputationMeasurements,
    Service,
    ServiceId,
    ServiceRevenue,
    TotalServed,
    TransactionRequest,
    TransactionResponse,
    TxHash,
    Value,
};
use lightning_interfaces::PagingParams;

use crate::state::State;
use crate::storage::AtomoStorage;
use crate::table::StateTables;

macro_rules! do_keys {
    ($self:ident, $table:ident, $ctx:expr) => {
        $self.$table.get($ctx).keys()
    };
}

macro_rules! do_query {
    ($self:ident, $table:ident, $ctx:expr, $key:expr) => {
        $self.$table.get($ctx).get($key)
    };
    ($self:ident, $table:ident, $ctx:expr, $key:expr, $selector:expr) => {
        do_query!($self, $table, $ctx, $key).map($selector)
    };
}

macro_rules! query {
    ($self:ident, $table:ident, $key:expr) => {
        $self.inner.run(|ctx| do_query!($self, $table, &ctx, $key))
    };
    ($self:ident, $table:ident, $key:expr, $selector:expr) => {
        $self
            .inner
            .run(|ctx| do_query!($self, $table, &ctx, $key, $selector))
    };
}

#[derive(Clone)]
pub struct QueryRunner {
    inner: Atomo<QueryPerm, AtomoStorage>,
    metadata_table: ResolvedTableReference<Metadata, Value>,
    account_table: ResolvedTableReference<EthAddress, AccountInfo>,
    client_table: ResolvedTableReference<ClientPublicKey, EthAddress>,
    node_table: ResolvedTableReference<NodeIndex, NodeInfo>,
    pub_key_to_index: ResolvedTableReference<NodePublicKey, NodeIndex>,
    committee_table: ResolvedTableReference<Epoch, Committee>,
    services_table: ResolvedTableReference<ServiceId, Service>,
    param_table: ResolvedTableReference<ProtocolParams, u128>,
    current_epoch_served: ResolvedTableReference<NodeIndex, NodeServed>,
    rep_measurements: ResolvedTableReference<NodeIndex, Vec<ReportedReputationMeasurements>>,
    latencies: ResolvedTableReference<(NodeIndex, NodeIndex), Duration>,
    rep_scores: ResolvedTableReference<NodeIndex, u8>,
    _last_epoch_served: ResolvedTableReference<NodeIndex, NodeServed>,
    total_served_table: ResolvedTableReference<Epoch, TotalServed>,
    _service_revenue: ResolvedTableReference<ServiceId, ServiceRevenue>,
    _commodity_price: ResolvedTableReference<CommodityTypes, HpUfixed<6>>,
    _executed_digests_table: ResolvedTableReference<TxHash, ()>,
    uptime_table: ResolvedTableReference<NodeIndex, u8>,
    _cid_to_node: ResolvedTableReference<Blake3Hash, BTreeSet<NodeIndex>>,
    _node_to_cid: ResolvedTableReference<NodeIndex, BTreeSet<Blake3Hash>>,
}

impl QueryRunner {
    pub fn init(atomo: Atomo<QueryPerm, AtomoStorage>) -> Self {
        Self {
            metadata_table: atomo.resolve::<Metadata, Value>("metadata"),
            account_table: atomo.resolve::<EthAddress, AccountInfo>("account"),
            client_table: atomo.resolve::<ClientPublicKey, EthAddress>("client_keys"),
            node_table: atomo.resolve::<NodeIndex, NodeInfo>("node"),
            pub_key_to_index: atomo.resolve::<NodePublicKey, NodeIndex>("pub_key_to_index"),
            committee_table: atomo.resolve::<Epoch, Committee>("committee"),
            services_table: atomo.resolve::<ServiceId, Service>("service"),
            param_table: atomo.resolve::<ProtocolParams, u128>("parameter"),
            current_epoch_served: atomo.resolve::<NodeIndex, NodeServed>("current_epoch_served"),
            rep_measurements: atomo
                .resolve::<NodeIndex, Vec<ReportedReputationMeasurements>>("rep_measurements"),
            latencies: atomo.resolve::<(NodeIndex, NodeIndex), Duration>("latencies"),
            rep_scores: atomo.resolve::<NodeIndex, u8>("rep_scores"),
            _last_epoch_served: atomo.resolve::<NodeIndex, NodeServed>("last_epoch_served"),
            total_served_table: atomo.resolve::<Epoch, TotalServed>("total_served"),
            _commodity_price: atomo.resolve::<CommodityTypes, HpUfixed<6>>("commodity_prices"),
            _service_revenue: atomo.resolve::<ServiceId, ServiceRevenue>("service_revenue"),
            _executed_digests_table: atomo.resolve::<TxHash, ()>("executed_digests"),
            uptime_table: atomo.resolve::<NodeIndex, u8>("uptime"),
            _cid_to_node: atomo.resolve::<Blake3Hash, BTreeSet<NodeIndex>>("cid_to_node"),
            _node_to_cid: atomo.resolve::<NodeIndex, BTreeSet<Blake3Hash>>("node_to_cid"),
            inner: atomo,
        }
    }
}

impl SyncQueryRunnerInterface for QueryRunner {
    fn get_metadata(&self, key: &Metadata) -> Option<Value> {
        query!(self, metadata_table, key)
    }

    fn get_account_info<F: Clone>(
        &self,
        address: &EthAddress,
        selector: impl FnOnce(AccountInfo) -> F,
    ) -> Option<F> {
        query!(self, account_table, address, selector)
    }

    fn get_client_info(self, pub_key: &ClientPublicKey) -> Option<EthAddress> {
        query!(self, client_table, pub_key)
    }

    fn get_node_info<F: Clone>(
        &self,
        node: &NodeIndex,
        selector: impl FnOnce(NodeInfo) -> F,
    ) -> Option<F> {
        query!(self, node_table, node, selector)
    }

    fn pubkey_to_index(&self, pub_key: &NodePublicKey) -> Option<NodeIndex> {
        query!(self, pub_key_to_index, pub_key)
    }

    fn get_committe_info<F: Clone>(
        &self,
        epoch: &Epoch,
        selector: impl FnOnce(Committee) -> F,
    ) -> Option<F> {
        query!(self, committee_table, epoch, selector)
    }

    fn get_service_info(&self, id: &ServiceId) -> Option<Service> {
        query!(self, services_table, id)
    }

    fn get_protocol_param(&self, param: &ProtocolParams) -> Option<u128> {
        query!(self, param_table, param)
    }

    fn get_current_epoch_served(&self, node: &NodeIndex) -> Option<NodeServed> {
        query!(self, current_epoch_served, node)
    }

    fn get_reputation_measurements(
        &self,
        node: &NodeIndex,
    ) -> Option<Vec<ReportedReputationMeasurements>> {
        query!(self, rep_measurements, node)
    }

    fn get_latencies(&self, node_1: &NodeIndex, node_2: &NodeIndex) -> Option<Duration> {
        query!(self, latencies, &(*node_1, *node_2))
    }

    fn get_reputation_score(&self, node: &NodeIndex) -> Option<u8> {
        query!(self, rep_scores, node)
    }

    fn get_total_served(&self, epoch: &Epoch) -> Option<TotalServed> {
        query!(self, total_served_table, epoch)
    }

    /// Returns the committee members of the current epoch
    #[autometrics]
    fn get_committee_members(&self) -> Vec<NodePublicKey> {
        self.get_committee_members_by_index()
            .into_iter()
            .filter_map(|node_index| self.index_to_pubkey(&node_index))
            .collect()
    }

    fn get_committee_members_by_index(&self) -> Vec<NodeIndex> {
        let current_epoch = self.get_current_epoch();

        self.get_committe_info(&current_epoch, |committee| committee.members)
            .unwrap_or_default()
    }

    fn get_current_epoch(&self) -> Epoch {
        match self.get_metadata(&Metadata::Epoch) {
            Some(Value::Epoch(epoch)) => epoch,
            _ => 0,
        }
    }

    fn get_epoch_info(&self) -> EpochInfo {
        let current_epoch = self.get_current_epoch();

        let committee = self
            .get_committe_info::<Committee>(&current_epoch, |c| c)
            .unwrap_or_default();

        EpochInfo {
            committee: committee
                .members
                .iter()
                .filter_map(|member| self.get_node_info::<NodeInfo>(member, |n| n))
                .collect(),
            epoch: current_epoch,
            epoch_end: committee.epoch_end_timestamp,
        }
    }

    fn get_current_latencies(&self) -> HashMap<(NodePublicKey, NodePublicKey), Duration> {
        self.inner.run(|ctx| {
            do_keys!(self, latencies, ctx)
                .filter_map(|key| {
                    do_query!(self, latencies, ctx, &key).map(|latency| (key, latency))
                })
                .filter_map(|((index_lhs, index_rhs), latency)| {
                    let pub_key_selector = { |n: NodeInfo| n.public_key };
                    let node_lhs = do_query!(self, node_table, ctx, &index_lhs, pub_key_selector);
                    let node_rhs = do_query!(self, node_table, ctx, &index_rhs, pub_key_selector);
                    match (node_lhs, node_rhs) {
                        (Some(node_lhs), Some(node_rhs)) => Some(((node_lhs, node_rhs), latency)),
                        _ => None,
                    }
                })
                .collect()
        })
    }

    fn get_genesis_committee(&self) -> Vec<(NodeIndex, NodeInfo)> {
        self.inner.run(|ctx| {
            match do_query!(self, metadata_table, ctx, &Metadata::GenesisCommittee) {
                Some(Value::GenesisCommittee(committee)) => committee
                    .iter()
                    .filter_map(|index| {
                        do_query!(self, node_table, ctx, index).map(|node_info| (*index, node_info))
                    })
                    .collect(),
                _ => {
                    // unreachable seeded at genesis
                    Vec::new()
                },
            }
        })
    }

    fn get_node_registry(&self, paging: Option<PagingParams>) -> Vec<NodeInfoWithIndex> {
        self.inner.run(|ctx| {
            let staking_amount: HpUfixed<18> =
                do_query!(self, param_table, ctx, ProtocolParams::MinimumNodeStake)
                    .unwrap_or(0)
                    .into();
            let nodes = do_keys!(self, node_table, ctx).map(|index| NodeInfoWithIndex {
                index,
                info: do_query!(self, node_table, ctx, &index).unwrap(),
            });

            match paging {
                None => nodes
                    .filter(|node| node.info.stake.staked >= staking_amount)
                    .collect(),
                Some(PagingParams {
                    ignore_stake,
                    limit,
                    start,
                }) => {
                    let mut nodes = nodes
                        .filter(|node| ignore_stake || node.info.stake.staked >= staking_amount)
                        .collect::<Vec<NodeInfoWithIndex>>();

                    nodes.sort_by_key(|info| info.index);

                    nodes
                        .into_iter()
                        .filter(|info| info.index >= start)
                        .take(limit)
                        .collect()
                },
            }
        })
    }

    fn index_to_pubkey(&self, node_index: &NodeIndex) -> Option<NodePublicKey> {
        self.get_node_info::<NodePublicKey>(node_index, |node_info| node_info.public_key)
    }

    fn is_valid_node(&self, id: &NodePublicKey) -> bool {
        self.inner.run(|ctx| {
            do_query!(self, pub_key_to_index, ctx, id).is_some_and(|node_idx| {
                do_query!(self, node_table, ctx, &node_idx, |n| n.stake.staked).is_some_and(
                    |node_stake| {
                        node_stake
                            >= do_query!(self, param_table, ctx, &ProtocolParams::MinimumNodeStake)
                                .unwrap_or(0)
                                .into()
                    },
                )
            })
        })
    }
    fn simulate_txn(&self, txn: TransactionRequest) -> TransactionResponse {
        self.inner.run(|ctx| {
            // Create the app/execution environment
            let backend = StateTables {
                table_selector: ctx,
            };
            let app = State::new(backend);
            app.execute_transaction(txn.clone())
        })
    }

    fn get_node_uptime(&self, node_index: &NodeIndex) -> Option<u8> {
        query!(self, uptime_table, node_index)
    }

    fn cid_to_providers(&self, cid: &Blake3Hash) -> Vec<NodeIndex> {
        // Todo: Optimize this search.
        self.inner.run(|ctx| {
            self._cid_to_node
                .get(ctx)
                .get(cid)
                .map(|nodes| nodes.into_iter().collect::<Vec<_>>())
                .unwrap_or_default()
        })
    }

    fn content_registry(&self, node_index: &NodeIndex) -> Vec<Blake3Hash> {
        self.inner.run(|ctx| {
            self._node_to_cid
                .get(ctx)
                .get(node_index)
                .map(|nodes| nodes.into_iter().collect::<Vec<_>>())
                .unwrap_or_default()
        })
    }
}
