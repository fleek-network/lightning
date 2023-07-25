use std::{collections::HashMap, time::Duration};

use atomo::{Atomo, QueryPerm, ResolvedTableReference};
use draco_interfaces::{
    application::SyncQueryRunnerInterface,
    types::{
        AccountInfo, CommodityTypes, Epoch, EpochInfo, Metadata, NodeInfo, NodeServed,
        ProtocolParams, ReportedReputationMeasurements, Service, ServiceId, ServiceRevenue,
        TotalServed, TransactionResponse, UpdateRequest, Value,
    },
};
use fleek_crypto::{ClientPublicKey, EthAddress, NodePublicKey};
use hp_float::unsigned::HpUfloat;

use crate::{
    state::{Committee, State},
    table::StateTables,
};

#[derive(Clone)]
pub struct QueryRunner {
    inner: Atomo<QueryPerm>,
    metadata_table: ResolvedTableReference<Metadata, Value>,
    account_table: ResolvedTableReference<EthAddress, AccountInfo>,
    client_table: ResolvedTableReference<ClientPublicKey, EthAddress>,
    node_table: ResolvedTableReference<NodePublicKey, NodeInfo>,
    committee_table: ResolvedTableReference<Epoch, Committee>,
    services_table: ResolvedTableReference<ServiceId, Service>,
    param_table: ResolvedTableReference<ProtocolParams, u128>,
    current_epoch_served: ResolvedTableReference<NodePublicKey, NodeServed>,
    rep_measurements: ResolvedTableReference<NodePublicKey, Vec<ReportedReputationMeasurements>>,
    latencies: ResolvedTableReference<(NodePublicKey, NodePublicKey), Duration>,
    rep_scores: ResolvedTableReference<NodePublicKey, u8>,
    _last_epoch_served: ResolvedTableReference<NodePublicKey, NodeServed>,
    total_served_table: ResolvedTableReference<Epoch, TotalServed>,
    _service_revenue: ResolvedTableReference<ServiceId, ServiceRevenue>,
    _commodity_price: ResolvedTableReference<CommodityTypes, HpUfloat<6>>,
}

impl QueryRunner {
    pub fn init(atomo: Atomo<QueryPerm>) -> Self {
        Self {
            metadata_table: atomo.resolve::<Metadata, Value>("metadata"),
            account_table: atomo.resolve::<EthAddress, AccountInfo>("account"),
            client_table: atomo.resolve::<ClientPublicKey, EthAddress>("client_keys"),
            node_table: atomo.resolve::<NodePublicKey, NodeInfo>("node"),
            committee_table: atomo.resolve::<Epoch, Committee>("committee"),
            services_table: atomo.resolve::<ServiceId, Service>("service"),
            param_table: atomo.resolve::<ProtocolParams, u128>("parameter"),
            current_epoch_served: atomo
                .resolve::<NodePublicKey, NodeServed>("current_epoch_served"),
            rep_measurements: atomo
                .resolve::<NodePublicKey, Vec<ReportedReputationMeasurements>>("rep_measurements"),
            latencies: atomo.resolve::<(NodePublicKey, NodePublicKey), Duration>("latencies"),
            rep_scores: atomo.resolve::<NodePublicKey, u8>("rep_scores"),
            _last_epoch_served: atomo.resolve::<NodePublicKey, NodeServed>("last_epoch_served"),
            total_served_table: atomo.resolve::<Epoch, TotalServed>("total_served"),
            _commodity_price: atomo.resolve::<CommodityTypes, HpUfloat<6>>("commodity_prices"),
            _service_revenue: atomo.resolve::<ServiceId, ServiceRevenue>("service_revenue"),
            inner: atomo,
        }
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

    fn get_flk_balance(&self, account: &EthAddress) -> HpUfloat<18> {
        self.inner.run(|ctx| {
            self.account_table
                .get(ctx)
                .get(account)
                .map(|account| account.flk_balance)
                .unwrap_or(HpUfloat::<18>::zero())
        })
    }

    fn get_stables_balance(&self, account: &EthAddress) -> HpUfloat<6> {
        self.inner.run(|ctx| {
            self.account_table
                .get(ctx)
                .get(account)
                .map(|account| account.stables_balance)
                .unwrap_or(HpUfloat::<6>::zero())
        })
    }

    fn get_staked(&self, node: &NodePublicKey) -> HpUfloat<18> {
        self.inner.run(|ctx| {
            self.node_table
                .get(ctx)
                .get(node)
                .map(|node| node.stake.staked)
                .unwrap_or(HpUfloat::zero())
        })
    }

    fn get_locked(&self, node: &NodePublicKey) -> HpUfloat<18> {
        self.inner.run(|ctx| {
            self.node_table
                .get(ctx)
                .get(node)
                .map(|node| node.stake.locked)
                .unwrap_or(HpUfloat::zero())
        })
    }

    fn get_stake_locked_until(&self, node: &NodePublicKey) -> Epoch {
        self.inner.run(|ctx| {
            self.node_table
                .get(ctx)
                .get(node)
                .map(|node| node.stake.stake_locked_until)
                .unwrap_or(0)
        })
    }

    fn get_locked_time(&self, node: &NodePublicKey) -> Epoch {
        self.inner.run(|ctx| {
            self.node_table
                .get(ctx)
                .get(node)
                .map(|node| node.stake.locked_until)
                .unwrap_or(0)
        })
    }

    fn get_rep_measurements(&self, node: NodePublicKey) -> Vec<ReportedReputationMeasurements> {
        self.inner
            .run(|ctx| self.rep_measurements.get(ctx).get(node).unwrap_or(vec![]))
    }

    fn get_reputation(&self, node: &NodePublicKey) -> Option<u8> {
        self.inner.run(|ctx| self.rep_scores.get(ctx).get(node))
    }

    fn get_relative_score(&self, _n1: &NodePublicKey, _n2: &NodePublicKey) -> u128 {
        todo!()
    }

    fn get_node_info(&self, id: &NodePublicKey) -> Option<NodeInfo> {
        self.inner.run(|ctx| self.node_table.get(ctx).get(id))
    }

    fn get_node_registry(&self) -> Vec<NodeInfo> {
        let public_keys: Vec<NodePublicKey> = self
            .inner
            .run(|ctx| self.node_table.get(ctx).keys())
            .collect();
        public_keys
            .into_iter()
            .filter(|node| self.is_valid_node(node))
            .filter_map(|node| self.inner.run(|ctx| self.node_table.get(ctx).get(node)))
            .collect()
    }

    fn is_valid_node(&self, id: &NodePublicKey) -> bool {
        // TODO(matthias): we can use `is_some_and` once we update the rust version to 1.70
        if let Some(node_info) = self.get_node_info(id) {
            node_info.stake.staked >= self.get_staking_amount().into()
        } else {
            false
        }
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
            self.current_epoch_served
                .get(ctx)
                .get(node)
                .unwrap_or_default()
        })
    }

    fn get_total_supply(&self) -> HpUfloat<18> {
        self.inner.run(|ctx| {
            let supply = match self.metadata_table.get(ctx).get(&Metadata::TotalSupply) {
                Some(Value::HpUfloat(s)) => s,
                _ => panic!("TotalSupply is set genesis and should never be empty"),
            };
            supply
        })
    }
    fn get_year_start_supply(&self) -> HpUfloat<18> {
        self.inner.run(|ctx| {
            let supply = match self.metadata_table.get(ctx).get(&Metadata::SupplyYearStart) {
                Some(Value::HpUfloat(s)) => s,
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
        let keys: Vec<(NodePublicKey, NodePublicKey)> = self
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
            .collect()
    }
    fn get_service_info(&self, service_id: ServiceId) -> Service {
        self.inner
            .run(|ctx| self.services_table.get(ctx).get(service_id).unwrap())
    }
}
