use std::collections::{BTreeSet, HashMap};
use std::time::Duration;

use atomo::{Atomo, QueryPerm, ResolvedTableReference};
use autometrics::autometrics;
use fleek_crypto::{ClientPublicKey, EthAddress, NodePublicKey};
use hp_fixed::unsigned::HpUfixed;
use lightning_interfaces::application::{QueryRunnerInterface, SyncQueryRunnerInterface};
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

macro_rules! query {
    ($self:ident, $table:ident, $key:expr) => {
        $self.inner.run(|ctx| $self.$table.get(ctx).get($key))
    };
    ($self:ident, $table:ident, $key:expr, $selector:expr) => {
        query!($self, $table, $key).map($selector)
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

    fn get_node_info_with_pub_key<F: FnOnce(NodeInfo) -> T, T>(
        &self,
        public_key: &NodePublicKey,
        f: F,
    ) -> Option<T> {
        self.inner.run(|ctx| {
            self.pub_key_to_index
                .get(ctx)
                .get(public_key)
                .map(|index| self.node_table.get(ctx).get(index).map(f))
                .unwrap_or(None)
        })
    }
}

impl QueryRunnerInterface for QueryRunner {
    fn get_metadata(&self, key: &Metadata) -> Option<Value> {
        query!(self, metadata_table, key)
    }

    fn get_account_info<T: Clone>(
        &self,
        address: &EthAddress,
        selector: impl FnOnce(AccountInfo) -> T,
    ) -> Option<T> {
        query!(self, account_table, address, selector)
    }

    fn get_client_info(self, pub_key: &ClientPublicKey) -> Option<EthAddress> {
        query!(self, client_table, pub_key)
    }

    fn get_node_info<T: Clone>(
        &self,
        node: &NodeIndex,
        selector: impl FnOnce(NodeInfo) -> T,
    ) -> Option<T> {
        query!(self, node_table, node, selector)
    }

    fn pub_key_to_index(&self, pub_key: &NodePublicKey) -> Option<NodeIndex> {
        query!(self, pub_key_to_index, pub_key)
    }

    fn get_committe_info<T: Clone>(
        &self,
        epoch: &Epoch,
        selector: impl FnOnce(Committee) -> T,
    ) -> Option<T> {
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

    /// Autometrics
    /// Returns the committee members of the current epoch
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
            app.execute_transaction(txn.clone())
        })
    }
}

impl SyncQueryRunnerInterface for QueryRunner {
    fn get_account_balance(&self, account: &EthAddress) -> u128 {
        self.inner.run(|ctx| {
            self.account_table
                .get(ctx)
                .get(account)
                .map(|a| a.bandwidth_balance)
                .unwrap_or(0)
        })
    }
    fn get_client_balance(&self, client: &ClientPublicKey) -> u128 {
        self.inner.run(|ctx| {
            let client_table = self.client_table.get(ctx);
            let account_table = self.account_table.get(ctx);
            // Lookup the account key in the client->account table and then check the balance on the
            // account
            client_table
                .get(client)
                .and_then(|key| account_table.get(key))
                .map(|a| a.bandwidth_balance)
                .unwrap_or(0)
        })
    }

    fn get_flk_balance(&self, account: &EthAddress) -> HpUfixed<18> {
        self.inner.run(|ctx| {
            self.account_table
                .get(ctx)
                .get(account)
                .map(|account| account.flk_balance)
                .unwrap_or(HpUfixed::<18>::zero())
        })
    }

    fn get_stables_balance(&self, account: &EthAddress) -> HpUfixed<6> {
        self.inner.run(|ctx| {
            self.account_table
                .get(ctx)
                .get(account)
                .map(|account| account.stables_balance)
                .unwrap_or(HpUfixed::<6>::zero())
        })
    }

    fn get_staked(&self, node: &NodePublicKey) -> HpUfixed<18> {
        self.get_node_info_with_pub_key(node, |node_info| node_info.stake.staked)
            .unwrap_or(HpUfixed::zero())
    }

    fn get_locked(&self, node: &NodePublicKey) -> HpUfixed<18> {
        self.get_node_info_with_pub_key(node, |node_info| node_info.stake.locked)
            .unwrap_or(HpUfixed::zero())
    }

    fn get_stake_locked_until(&self, node: &NodePublicKey) -> Epoch {
        self.get_node_info_with_pub_key(node, |node_info| node_info.stake.stake_locked_until)
            .unwrap_or(0)
    }

    fn get_locked_time(&self, node: &NodePublicKey) -> Epoch {
        self.get_node_info_with_pub_key(node, |node_info| node_info.stake.locked_until)
            .unwrap_or(0)
    }

    fn get_rep_measurements(&self, node: &NodeIndex) -> Vec<ReportedReputationMeasurements> {
        self.inner
            .run(|ctx| self.rep_measurements.get(ctx).get(node).unwrap_or_default())
    }

    fn get_reputation(&self, node: &NodeIndex) -> Option<u8> {
        self.inner.run(|ctx| self.rep_scores.get(ctx).get(node))
    }

    fn get_relative_score(&self, _n1: &NodePublicKey, _n2: &NodePublicKey) -> u128 {
        todo!()
    }

    fn get_node_info_with_index(&self, node_index: &NodeIndex) -> Option<NodeInfo> {
        self.inner
            .run(|ctx| self.node_table.get(ctx).get(node_index))
    }

    fn get_account_info(&self, id: &EthAddress) -> Option<AccountInfo> {
        self.inner.run(|ctx| self.account_table.get(ctx).get(id))
    }

    fn get_node_registry(&self, paging: Option<PagingParams>) -> Vec<NodeInfo> {
        self.get_node_registry_with_index(paging)
            .into_iter()
            .map(|node| node.info)
            .collect()
    }

    fn get_node_registry_with_index(&self, paging: Option<PagingParams>) -> Vec<NodeInfoWithIndex> {
        let staking_amount: HpUfixed<18> = self.get_staking_amount().into();
        match paging {
            None => self.inner.run(|ctx| {
                let node_table = self.node_table.get(ctx);
                node_table
                    .keys()
                    .map(|index| NodeInfoWithIndex {
                        index,
                        info: node_table.get(index).unwrap(),
                    })
                    .filter(|node| node.info.stake.staked >= staking_amount)
                    .collect()
            }),
            Some(PagingParams {
                ignore_stake,
                limit,
                start,
            }) => self.inner.run(|ctx| {
                let node_table = self.node_table.get(ctx);
                let mut keys = node_table.keys().collect::<Vec<NodeIndex>>();
                // Keys are returned unsorted so sort them here.
                keys.sort();

                keys.into_iter()
                    .filter(|index| index >= &start)
                    .map(|index| NodeInfoWithIndex {
                        index,
                        info: node_table.get(index).unwrap(),
                    })
                    .filter(|node| ignore_stake || node.info.stake.staked >= staking_amount)
                    .take(limit)
                    .collect()
            }),
        }
    }

    fn is_valid_node(&self, id: &NodePublicKey) -> bool {
        self.pub_key_to_index(id).is_some_and(|id| {
            self.get_node_info::<HpUfixed<18>>(&id, |node_info| node_info.stake.staked)
                .is_some_and(|staked| staked >= self.get_staking_amount().into())
        })
    }

    fn get_staking_amount(&self) -> u128 {
        self.inner.run(|ctx| {
            self.param_table
                .get(ctx)
                .get(&ProtocolParams::MinimumNodeStake)
                .unwrap_or(0)
        })
    }

    fn get_epoch_randomness_seed(&self) -> &[u8; 32] {
        todo!()
    }

    #[autometrics]
    fn get_committee_members(&self) -> Vec<NodePublicKey> {
        self.inner.run(|ctx| {
            // get current epoch first
            let epoch = match self.metadata_table.get(ctx).get(&Metadata::Epoch) {
                Some(Value::Epoch(epoch)) => epoch,
                _ => 0,
            };

            // look up current committee
            self.committee_table
                .get(ctx)
                .get(epoch)
                .map(|c| c.members)
                .unwrap_or_default()
                .into_iter()
                .filter_map(|index| {
                    self.node_table
                        .get(ctx)
                        .get(index)
                        .map(|node| node.public_key)
                })
                .collect()
        })
    }

    fn get_epoch(&self) -> Epoch {
        self.inner.run(
            |ctx| match self.metadata_table.get(ctx).get(&Metadata::Epoch) {
                Some(Value::Epoch(epoch)) => epoch,
                _ => 0,
            },
        )
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

    fn get_total_served(&self, epoch: Epoch) -> TotalServed {
        self.inner.run(|ctx| {
            self.total_served_table
                .get(ctx)
                .get(epoch)
                .unwrap_or_default()
        })
    }

    fn get_node_served(&self, node: &NodePublicKey) -> NodeServed {
        self.inner.run(|ctx| {
            self.pub_key_to_index
                .get(ctx)
                .get(node)
                .map(|index| {
                    self.current_epoch_served
                        .get(ctx)
                        .get(index)
                        .unwrap_or_default()
                })
                .unwrap_or_default()
        })
    }

    fn get_total_supply(&self) -> HpUfixed<18> {
        self.inner.run(|ctx| {
            let supply = match self.metadata_table.get(ctx).get(&Metadata::TotalSupply) {
                Some(Value::HpUfixed(s)) => s,
                _ => panic!("TotalSupply is set genesis and should never be empty"),
            };
            supply
        })
    }
    fn get_year_start_supply(&self) -> HpUfixed<18> {
        self.inner.run(|ctx| {
            let supply = match self.metadata_table.get(ctx).get(&Metadata::SupplyYearStart) {
                Some(Value::HpUfixed(s)) => s,
                _ => panic!("SupplyYearStart is set genesis and should never be empty"),
            };
            supply
        })
    }

    fn get_protocol_fund_address(&self) -> EthAddress {
        self.inner.run(|ctx| {
            let owner = match self
                .metadata_table
                .get(ctx)
                .get(&Metadata::ProtocolFundAddress)
            {
                Some(Value::AccountPublicKey(s)) => s,
                _ => panic!("AccountPublicKey is set genesis and should never be empty"),
            };
            owner
        })
    }

    fn get_protocol_params(&self, param: ProtocolParams) -> u128 {
        self.inner.run(|ctx| {
            let param = &param;
            self.param_table.get(ctx).get(param).unwrap_or(0)
        })
    }

    fn validate_txn(&self, txn: TransactionRequest) -> TransactionResponse {
        self.inner.run(|ctx| {
            // Create the app/execution environment
            let backend = StateTables {
                table_selector: ctx,
            };
            let app = State::new(backend);
            app.execute_transaction(txn.clone())
        })
    }

    fn get_latencies(&self) -> HashMap<(NodePublicKey, NodePublicKey), Duration> {
        let keys: Vec<(u32, u32)> = self
            .inner
            .run(|ctx| self.latencies.get(ctx).keys())
            .collect();

        keys.into_iter()
            .filter_map(|key| {
                self.inner.run(|ctx| {
                    self.latencies
                        .get(ctx)
                        .get(key)
                        .map(|latency| (key, latency))
                })
            })
            .filter_map(|((index_lhs, index_rhs), latency)| {
                let node_lhs = self.index_to_pubkey(&index_lhs);
                let node_rhs = self.index_to_pubkey(&index_rhs);
                match (node_lhs, node_rhs) {
                    (Some(node_lhs), Some(node_rhs)) => Some(((node_lhs, node_rhs), latency)),
                    _ => None,
                }
            })
            .collect()
    }
    fn get_service_info(&self, service_id: ServiceId) -> Service {
        self.inner
            .run(|ctx| self.services_table.get(ctx).get(service_id).unwrap())
    }

    fn get_last_epoch_hash(&self) -> [u8; 32] {
        self.inner.run(
            |ctx| match self.metadata_table.get(ctx).get(&Metadata::LastEpochHash) {
                Some(Value::Hash(hash)) => hash,
                _ => [0; 32],
            },
        )
    }

    fn is_committee(&self, node_index: u32) -> bool {
        self.inner.run(|ctx| {
            // get current epoch
            let epoch = match self.metadata_table.get(ctx).get(&Metadata::Epoch) {
                Some(Value::Epoch(epoch)) => epoch,
                _ => 0,
            };

            self.committee_table
                .get(ctx)
                .get(epoch)
                .map(|c| c.members.contains(&node_index))
                .unwrap_or(false)
        })
    }

    fn get_last_block(&self) -> [u8; 32] {
        self.inner.run(
            |ctx| match self.metadata_table.get(ctx).get(&Metadata::LastBlockHash) {
                Some(Value::Hash(hash)) => hash,
                _ => [0; 32],
            },
        )
    }

    fn genesis_committee(&self) -> Vec<(NodeIndex, NodeInfo)> {
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

    fn get_account_nonce(&self, public_key: &EthAddress) -> u64 {
        self.inner.run(|ctx| {
            self.account_table
                .get(ctx)
                .get(public_key)
                .map(|account| account.nonce)
                .unwrap_or_default()
        })
    }

    fn get_chain_id(&self) -> u32 {
        self.inner.run(
            |ctx| match self.metadata_table.get(ctx).get(&Metadata::ChainId) {
                Some(Value::ChainId(id)) => id,
                _ => 0,
            },
        )
    }

    fn get_block_number(&self) -> u64 {
        self.inner.run(
            |ctx| match self.metadata_table.get(ctx).get(&Metadata::BlockNumber) {
                Some(Value::BlockNumber(num)) => num,
                _ => 0,
            },
        )
    }

    fn has_executed_digest(&self, digest: [u8; 32]) -> bool {
        self.inner
            .run(|ctx| self.executed_digests_table.get(ctx).get(digest).is_some())
    }

    fn get_node_uptime(&self, node_index: &NodeIndex) -> Option<u8> {
        self.inner
            .run(|ctx| self.uptime_table.get(ctx).get(node_index))
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
