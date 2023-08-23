use std::collections::HashMap;
use std::time::Duration;

use atomo::{Atomo, QueryPerm, ResolvedTableReference};
use autometrics::autometrics;
use fleek_crypto::{ClientPublicKey, EthAddress, NodePublicKey};
use hp_fixed::unsigned::HpUfixed;
use lightning_interfaces::application::SyncQueryRunnerInterface;
use lightning_interfaces::types::{
    AccountInfo,
    Committee,
    CommodityTypes,
    Epoch,
    EpochInfo,
    Metadata,
    NodeIndex,
    NodeInfo,
    NodeServed,
    ProtocolParams,
    ReportedReputationMeasurements,
    Service,
    ServiceId,
    ServiceRevenue,
    TotalServed,
    TransactionResponse,
    UpdateRequest,
    Value,
};

use crate::state::State;
use crate::table::StateTables;

#[derive(Clone)]
pub struct QueryRunner {
    inner: Atomo<QueryPerm>,
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
}

impl QueryRunner {
    pub fn init(atomo: Atomo<QueryPerm>) -> Self {
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

    fn get_rep_measurements(&self, node: NodePublicKey) -> Vec<ReportedReputationMeasurements> {
        self.inner.run(|ctx| {
            self.pub_key_to_index
                .get(ctx)
                .get(node)
                .map(|index| self.rep_measurements.get(ctx).get(index).unwrap_or(vec![]))
                .unwrap_or(vec![])
        })
    }

    fn get_reputation(&self, node: &NodePublicKey) -> Option<u8> {
        self.inner.run(|ctx| {
            self.pub_key_to_index
                .get(ctx)
                .get(node)
                .and_then(|index| self.rep_scores.get(ctx).get(index))
        })
    }

    fn get_relative_score(&self, _n1: &NodePublicKey, _n2: &NodePublicKey) -> u128 {
        todo!()
    }

    fn get_node_info(&self, id: &NodePublicKey) -> Option<NodeInfo> {
        self.get_node_info_with_pub_key(id, |node_info| node_info)
    }

    fn get_node_registry(&self) -> Vec<NodeInfo> {
        let staking_amount: HpUfixed<18> = self.get_staking_amount().into();
        self.inner.run(|ctx| {
            let node_table = self.node_table.get(ctx);
            node_table
                .keys()
                .map(|index| node_table.get(index).unwrap())
                .filter(|node| node.stake.staked >= staking_amount)
                .collect()
        })
    }

    fn is_valid_node(&self, id: &NodePublicKey) -> bool {
        self.get_node_info(id)
            .is_some_and(|node_info| node_info.stake.staked >= self.get_staking_amount().into())
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

    fn validate_txn(&self, txn: UpdateRequest) -> TransactionResponse {
        self.inner.run(|ctx| {
            // Create the app/execution enviroment
            let backend = StateTables {
                table_selector: ctx,
            };
            let app = State::new(backend);
            app.execute_txn(txn.clone())
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
                let node_lhs = self.index_to_pubkey(index_lhs);
                let node_rhs = self.index_to_pubkey(index_rhs);
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

    fn pubkey_to_index(&self, node: NodePublicKey) -> Option<NodeIndex> {
        self.inner
            .run(|ctx| self.pub_key_to_index.get(ctx).get(node))
    }

    fn index_to_pubkey(&self, node_index: NodeIndex) -> Option<NodePublicKey> {
        self.inner.run(|ctx| {
            self.node_table
                .get(ctx)
                .get(node_index)
                .map(|info| info.public_key)
        })
    }
}
