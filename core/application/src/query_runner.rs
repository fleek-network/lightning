use atomo::{Atomo, QueryPerm, ResolvedTableReference};
use big_decimal::BigDecimal;
use draco_interfaces::{
    application::SyncQueryRunnerInterface,
    types::{
        AccountInfo, CommodityServed, CommodityTypes, Epoch, EpochInfo, Metadata, NodeInfo,
        ProtocolParams, ReportedReputationMeasurements, Service, ServiceId, TotalServed,
    },
};
use fleek_crypto::{AccountOwnerPublicKey, ClientPublicKey, NodePublicKey};

use crate::state::Committee;

#[derive(Clone)]
pub struct QueryRunner {
    inner: Atomo<QueryPerm>,
    metadata_table: ResolvedTableReference<Metadata, BigDecimal<18>>,
    account_table: ResolvedTableReference<AccountOwnerPublicKey, AccountInfo>,
    client_table: ResolvedTableReference<ClientPublicKey, AccountOwnerPublicKey>,
    node_table: ResolvedTableReference<NodePublicKey, NodeInfo>,
    committee_table: ResolvedTableReference<Epoch, Committee>,
    _services_table: ResolvedTableReference<ServiceId, Service>,
    param_table: ResolvedTableReference<ProtocolParams, String>,
    current_epoch_served: ResolvedTableReference<NodePublicKey, CommodityServed>,
    rep_measurements: ResolvedTableReference<NodePublicKey, Vec<ReportedReputationMeasurements>>,
    rep_scores: ResolvedTableReference<NodePublicKey, u8>,
    _last_epoch_served: ResolvedTableReference<NodePublicKey, CommodityServed>,
    total_served_table: ResolvedTableReference<Epoch, TotalServed>,
    _commodity_price: ResolvedTableReference<CommodityTypes, f64>,
}

impl QueryRunner {
    pub fn init(atomo: Atomo<QueryPerm>) -> Self {
        Self {
            metadata_table: atomo.resolve::<Metadata, BigDecimal<18>>("metadata"),
            account_table: atomo.resolve::<AccountOwnerPublicKey, AccountInfo>("account"),
            client_table: atomo.resolve::<ClientPublicKey, AccountOwnerPublicKey>("client_keys"),
            node_table: atomo.resolve::<NodePublicKey, NodeInfo>("node"),
            committee_table: atomo.resolve::<Epoch, Committee>("committee"),
            _services_table: atomo.resolve::<ServiceId, Service>("service"),
            param_table: atomo.resolve::<ProtocolParams, String>("parameter"),
            current_epoch_served: atomo
                .resolve::<NodePublicKey, CommodityServed>("current_epoch_served"),
            rep_measurements: atomo
                .resolve::<NodePublicKey, Vec<ReportedReputationMeasurements>>("rep_measurements"),
            rep_scores: atomo.resolve::<NodePublicKey, u8>("rep_scores"),
            _last_epoch_served: atomo
                .resolve::<NodePublicKey, CommodityServed>("last_epoch_served"),
            total_served_table: atomo.resolve::<Epoch, TotalServed>("total_served"),
            _commodity_price: atomo.resolve::<CommodityTypes, f64>("commodity_prices"),
            inner: atomo,
        }
    }
}

impl SyncQueryRunnerInterface for QueryRunner {
    fn get_account_balance(&self, account: &AccountOwnerPublicKey) -> u128 {
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

    fn get_flk_balance(&self, account: &AccountOwnerPublicKey) -> BigDecimal<18> {
        self.inner.run(|ctx| {
            self.account_table
                .get(ctx)
                .get(account)
                .map(|account| account.flk_balance)
                .unwrap_or(BigDecimal::<18>::zero())
        })
    }

    fn get_stables_balance(&self, account: &AccountOwnerPublicKey) -> BigDecimal<6> {
        self.inner.run(|ctx| {
            self.account_table
                .get(ctx)
                .get(account)
                .map(|account| account.stables_balance)
                .unwrap_or(BigDecimal::<6>::zero())
        })
    }

    fn get_staked(&self, node: &NodePublicKey) -> BigDecimal<18> {
        self.inner.run(|ctx| {
            self.node_table
                .get(ctx)
                .get(node)
                .map(|node| node.stake.staked)
                .unwrap_or(BigDecimal::zero())
        })
    }

    fn get_locked(&self, node: &NodePublicKey) -> BigDecimal<18> {
        self.inner.run(|ctx| {
            self.node_table
                .get(ctx)
                .get(node)
                .map(|node| node.stake.locked)
                .unwrap_or(BigDecimal::zero())
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
        todo!()
    }

    fn is_valid_node(&self, _id: &NodePublicKey) -> bool {
        todo!()
    }

    fn get_staking_amount(&self) -> u128 {
        self.inner.run(|ctx| {
            self.param_table
                .get(ctx)
                .get(&ProtocolParams::MinimumNodeStake)
                .unwrap_or("0".to_owned())
                .parse()
                .unwrap_or(0)
        })
    }

    fn get_epoch_randomness_seed(&self) -> &[u8; 32] {
        todo!()
    }

    fn get_committee_members(&self) -> Vec<NodePublicKey> {
        self.inner.run(|ctx| {
            // get current epoch first
            let epoch: u64 = self
                .metadata_table
                .get(ctx)
                .get(&Metadata::Epoch)
                .unwrap_or_default()
                .try_into()
                .unwrap();

            // look up current committee
            self.committee_table
                .get(ctx)
                .get(epoch)
                .map(|c| c.members)
                .unwrap_or_default()
        })
    }

    fn get_epoch_info(&self) -> EpochInfo {
        self.inner.run(|ctx| {
            let node_table = self.node_table.get(ctx);

            // get current epoch
            let epoch = self
                .metadata_table
                .get(ctx)
                .get(&Metadata::Epoch)
                .unwrap_or_default()
                .try_into()
                .unwrap();
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

    fn get_commodity_served(&self, node: &NodePublicKey) -> CommodityServed {
        self.inner.run(|ctx| {
            self.current_epoch_served
                .get(ctx)
                .get(node)
                .unwrap_or_default()
        })
    }

    fn get_total_supply(&self) -> BigDecimal<18> {
        self.inner.run(|ctx| {
            self.metadata_table
                .get(ctx)
                .get(Metadata::TotalSupply)
                .unwrap_or_default()
        })
    }
    fn get_year_start_supply(&self) -> BigDecimal<18> {
        self.inner.run(|ctx| {
            self.metadata_table
                .get(ctx)
                .get(Metadata::SupplyYearStart)
                .unwrap_or_default()
        })
    }

    fn get_protocol_params(&self, param: ProtocolParams) -> String {
        self.inner.run(|ctx| {
            let param = &param;
            self.param_table
                .get(ctx)
                .get(param)
                .unwrap_or("0".to_owned())
        })
    }
}
