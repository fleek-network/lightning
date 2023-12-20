use std::collections::{BTreeSet, HashMap};
use std::time::Duration;

use atomo::{Atomo, KeyIterator, QueryPerm, ResolvedTableReference};
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
use lightning_interfaces::{PagingParams, QueryRunnerExt};

use crate::state::State;
use crate::storage::AtomoStorage;
use crate::table::StateTables;

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
    executed_digests_table: ResolvedTableReference<TxHash, ()>,
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
            executed_digests_table: atomo.resolve::<TxHash, ()>("executed_digests"),
            uptime_table: atomo.resolve::<NodeIndex, u8>("uptime"),
            _cid_to_node: atomo.resolve::<Blake3Hash, BTreeSet<NodeIndex>>("cid_to_node"),
            _node_to_cid: atomo.resolve::<NodeIndex, BTreeSet<Blake3Hash>>("node_to_cid"),
            inner: atomo,
        }
    }
}

impl SyncQueryRunnerInterface for QueryRunner {
    fn get_metadata(&self, key: &Metadata) -> Option<Value> {
        self.inner.run(|ctx| self.metadata_table.get(ctx).get(key))
    }

    #[inline]
    fn get_account_info<V>(
        &self,
        address: &EthAddress,
        selector: impl FnOnce(AccountInfo) -> V,
    ) -> Option<V> {
        self.inner
            .run(|ctx| self.account_table.get(ctx).get(address))
            .map(selector)
    }

    fn client_key_to_account_key(self, pub_key: &ClientPublicKey) -> Option<EthAddress> {
        self.inner
            .run(|ctx| self.client_table.get(ctx).get(pub_key))
    }

    #[inline]
    fn get_node_info<V>(
        &self,
        node: &NodeIndex,
        selector: impl FnOnce(NodeInfo) -> V,
    ) -> Option<V> {
        self.inner
            .run(|ctx| self.node_table.get(ctx).get(node))
            .map(selector)
    }

    fn get_node_table_iter<V>(&self, closure: impl FnOnce(KeyIterator<NodeIndex>) -> V) -> V {
        self.inner
            .run(|ctx| closure(self.node_table.get(ctx).keys()))
    }

    fn pubkey_to_index(&self, pub_key: &NodePublicKey) -> Option<NodeIndex> {
        self.inner
            .run(|ctx| self.pub_key_to_index.get(ctx).get(pub_key))
    }

    #[inline]
    fn get_committe_info<V>(
        &self,
        epoch: &Epoch,
        selector: impl FnOnce(Committee) -> V,
    ) -> Option<V> {
        self.inner
            .run(|ctx| self.committee_table.get(ctx).get(epoch))
            .map(selector)
    }

    fn get_service_info(&self, id: &ServiceId) -> Option<Service> {
        self.inner.run(|ctx| self.services_table.get(ctx).get(id))
    }

    fn get_protocol_param(&self, param: &ProtocolParams) -> Option<u128> {
        self.inner.run(|ctx| self.param_table.get(ctx).get(param))
    }

    fn get_current_epoch_served(&self, node: &NodeIndex) -> Option<NodeServed> {
        self.inner
            .run(|ctx| self.current_epoch_served.get(ctx).get(node))
    }

    fn get_reputation_measurements(
        &self,
        node: &NodeIndex,
    ) -> Option<Vec<ReportedReputationMeasurements>> {
        self.inner
            .run(|ctx| self.rep_measurements.get(ctx).get(node))
    }

    fn get_latencies(&self, node_1: &NodeIndex, node_2: &NodeIndex) -> Option<Duration> {
        self.inner
            .run(|ctx| self.latencies.get(ctx).get((*node_1, *node_2)))
    }

    fn get_reputation_score(&self, node: &NodeIndex) -> Option<u8> {
        self.inner.run(|ctx| self.rep_scores.get(ctx).get(node))
    }

    fn get_total_served(&self, epoch: &Epoch) -> Option<TotalServed> {
        self.inner
            .run(|ctx| self.total_served_table.get(ctx).get(epoch))
    }

    fn has_executed_digest(&self, digest: [u8; 32]) -> bool {
        self.inner
            .run(|ctx| self.executed_digests_table.get(ctx).get(digest))
            .is_some()
    }

    fn index_to_pubkey(&self, node_index: &NodeIndex) -> Option<NodePublicKey> {
        self.get_node_info::<NodePublicKey>(node_index, |node_info| node_info.public_key)
    }

    fn simulate_txn(&self, txn: TransactionRequest) -> TransactionResponse {
        self.inner.run(|ctx| {
            // Create the app/execution environment
            let backend = StateTables {
                table_selector: ctx,
            };
            let app = State::new(backend);
            app.execute_transaction(txn)
        })
    }

    fn get_node_uptime(&self, node_index: &NodeIndex) -> Option<u8> {
        self.inner
            .run(|ctx| self.uptime_table.get(ctx).get(node_index))
    }

    fn get_cid_providers(&self, cid: &Blake3Hash) -> Option<BTreeSet<NodeIndex>> {
        self.inner.run(|ctx| self._cid_to_node.get(ctx).get(cid))
    }

    fn get_content_registry(&self, node_index: &NodeIndex) -> Option<BTreeSet<Blake3Hash>> {
        self.inner
            .run(|ctx| self._node_to_cid.get(ctx).get(node_index))
    }
}

impl QueryRunnerExt for QueryRunner {
    fn get_chain_id(&self) -> u32 {
        match self.get_metadata(&Metadata::ChainId) {
            Some(Value::ChainId(id)) => id,
            _ => 0,
        }
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
        self.inner.run(|ctx| {
            // get current epoch
            let epoch = match self.metadata_table.get(ctx).get(&Metadata::Epoch) {
                Some(Value::Epoch(epoch)) => epoch,
                _ => 0,
            };

            self.committee_table
                .get(ctx)
                .get(epoch)
                .map(|c| c.members)
                .unwrap_or_default()
        })
    }

    fn get_current_epoch(&self) -> Epoch {
        match self.get_metadata(&Metadata::Epoch) {
            Some(Value::Epoch(epoch)) => epoch,
            _ => 0,
        }
    }

    fn get_epoch_info(&self) -> EpochInfo {
        self.inner.run(|ctx| {
            let node_table = self.node_table.get(ctx);

            // get current epoch
            let epoch = match self.metadata_table.get(ctx).get(&Metadata::Epoch) {
                Some(Value::Epoch(epoch)) => epoch,
                _ => 0,
            };

            // look up current committee
            let committee = self.committee_table.get(ctx).get(epoch).unwrap_or_default();

            EpochInfo {
                committee: committee
                    .members
                    .iter()
                    .filter_map(|member| node_table.get(member))
                    .collect(),
                epoch,
                epoch_end: committee.epoch_end_timestamp,
            }
        })
    }

    fn get_current_latencies(&self) -> HashMap<(NodePublicKey, NodePublicKey), Duration> {
        let pub_key_selector = { |n: NodeInfo| n.public_key };
        self.inner.run(|ctx| {
            let latencies_table = self.latencies.get(ctx);
            let node_table = self.node_table.get(ctx);

            latencies_table
                .keys()
                .filter_map(|key| latencies_table.get(key).map(|latency| (key, latency)))
                .filter_map(|((index_lhs, index_rhs), latency)| {
                    let node_lhs = node_table.get(index_lhs).map(pub_key_selector);
                    let node_rhs = node_table.get(index_rhs).map(pub_key_selector);
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
            let node_table = self.node_table.get(ctx);

            match self
                .metadata_table
                .get(ctx)
                .get(&Metadata::GenesisCommittee)
            {
                Some(Value::GenesisCommittee(committee)) => committee
                    .iter()
                    .filter_map(|index| node_table.get(index).map(|node_info| (*index, node_info)))
                    .collect(),
                _ => {
                    // unreachable seeded at genesis
                    Vec::new()
                },
            }
        })
    }

    fn get_last_block(&self) -> [u8; 32] {
        match self.get_metadata(&Metadata::LastBlockHash) {
            Some(Value::Hash(hash)) => hash,
            _ => [0; 32],
        }
    }

    fn get_node_registry(&self, paging: Option<PagingParams>) -> Vec<NodeInfoWithIndex> {
        self.inner.run(|ctx| {
            let staking_amount: HpUfixed<18> = self.get_staking_amount().into();
            let node_table = self.node_table.get(ctx);

            let closure = {
                |nodes: KeyIterator<NodeIndex>| -> Vec<NodeInfoWithIndex> {
                    let nodes = nodes.map(|index| NodeInfoWithIndex {
                        index,
                        info: node_table.get(index).unwrap(),
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
                                .filter(|node| {
                                    ignore_stake || node.info.stake.staked >= staking_amount
                                })
                                .collect::<Vec<NodeInfoWithIndex>>();

                            nodes.sort_by_key(|info| info.index);

                            nodes
                                .into_iter()
                                .filter(|info| info.index >= start)
                                .take(limit)
                                .collect()
                        },
                    }
                }
            };

            self.get_node_table_iter::<Vec<NodeInfoWithIndex>>(closure)
        })
    }

    fn get_staking_amount(&self) -> u128 {
        self.get_protocol_param(&ProtocolParams::MinimumNodeStake)
            .unwrap_or(0)
    }

    fn is_valid_node(&self, id: &NodePublicKey) -> bool {
        let minimum_stake_amount = self.get_staking_amount().into();
        self.pubkey_to_index(id).is_some_and(|node_idx| {
            self.get_node_info(&node_idx, |n| n.stake.staked)
                .is_some_and(|node_stake| node_stake >= minimum_stake_amount)
        })
    }
}
