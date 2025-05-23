use std::collections::BTreeSet;
use std::path::Path;
use std::time::Duration;

use anyhow::Result;
use atomo::{
    Atomo,
    AtomoBuilder,
    DefaultSerdeBackend,
    KeyIterator,
    QueryPerm,
    ResolvedTableReference,
    TableSelector,
};
use fleek_crypto::{ClientPublicKey, EthAddress, NodePublicKey};
use fxhash::FxHashMap;
use hp_fixed::unsigned::HpUfixed;
use lightning_interfaces::types::{
    AccountInfo,
    Blake3Hash,
    Committee,
    CommitteeSelectionBeaconCommit,
    CommitteeSelectionBeaconReveal,
    CommodityTypes,
    Epoch,
    Job,
    Metadata,
    MintInfo,
    NodeIndex,
    NodeInfo,
    NodeServed,
    Nonce,
    ProtocolParamKey,
    ProtocolParamValue,
    ReportedReputationMeasurements,
    Service,
    ServiceId,
    ServiceRevenue,
    StateProofKey,
    StateProofValue,
    TotalServed,
    TransactionRequest,
    TransactionResponse,
    TxHash,
    Value,
    WithdrawInfo,
    WithdrawInfoWithId,
};
use lightning_interfaces::{SyncQueryRunnerInterface, WithdrawPagingParams};
use merklize::{StateRootHash, StateTree};

use crate::env::ApplicationStateTree;
use crate::state::ApplicationState;
use crate::storage::{AtomoStorage, AtomoStorageBuilder};

#[derive(Clone)]
pub struct QueryRunner {
    inner: Atomo<QueryPerm, AtomoStorage>,
    metadata_table: ResolvedTableReference<Metadata, Value>,
    account_table: ResolvedTableReference<EthAddress, AccountInfo>,
    client_table: ResolvedTableReference<ClientPublicKey, (EthAddress, Nonce)>,
    node_table: ResolvedTableReference<NodeIndex, NodeInfo>,
    pub_key_to_index: ResolvedTableReference<NodePublicKey, NodeIndex>,
    committee_table: ResolvedTableReference<Epoch, Committee>,
    services_table: ResolvedTableReference<ServiceId, Service>,
    param_table: ResolvedTableReference<ProtocolParamKey, ProtocolParamValue>,
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
    uri_to_node: ResolvedTableReference<Blake3Hash, BTreeSet<NodeIndex>>,
    node_to_uri: ResolvedTableReference<NodeIndex, BTreeSet<Blake3Hash>>,
    committee_selection_beacon: ResolvedTableReference<
        NodeIndex,
        (
            CommitteeSelectionBeaconCommit,
            Option<CommitteeSelectionBeaconReveal>,
        ),
    >,
    committee_selection_beacon_non_revealing_node: ResolvedTableReference<NodeIndex, ()>,
    withdraws: ResolvedTableReference<u64, WithdrawInfo>,
    assigned_jobs: ResolvedTableReference<NodeIndex, Vec<[u8; 32]>>,
    jobs: ResolvedTableReference<[u8; 32], Job>,
    mints: ResolvedTableReference<[u8; 32], MintInfo>,
}

impl QueryRunner {
    pub fn run<F, R>(&self, query: F) -> R
    where
        F: FnOnce(&mut TableSelector<AtomoStorage, DefaultSerdeBackend>) -> R,
    {
        self.inner.run(query)
    }
}

impl SyncQueryRunnerInterface for QueryRunner {
    type Backend = AtomoStorage;

    fn new(atomo: Atomo<QueryPerm, AtomoStorage>) -> Self {
        Self {
            metadata_table: atomo.resolve::<Metadata, Value>("metadata"),
            account_table: atomo.resolve::<EthAddress, AccountInfo>("account"),
            client_table: atomo.resolve::<ClientPublicKey, (EthAddress, Nonce)>("client_keys"),
            node_table: atomo.resolve::<NodeIndex, NodeInfo>("node"),
            pub_key_to_index: atomo.resolve::<NodePublicKey, NodeIndex>("pub_key_to_index"),
            committee_table: atomo.resolve::<Epoch, Committee>("committee"),
            services_table: atomo.resolve::<ServiceId, Service>("service"),
            param_table: atomo.resolve::<ProtocolParamKey, ProtocolParamValue>("parameter"),
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
            uri_to_node: atomo.resolve::<Blake3Hash, BTreeSet<NodeIndex>>("uri_to_node"),
            node_to_uri: atomo.resolve::<NodeIndex, BTreeSet<Blake3Hash>>("node_to_uri"),
            committee_selection_beacon: atomo.resolve::<NodeIndex, (
                CommitteeSelectionBeaconCommit,
                Option<CommitteeSelectionBeaconReveal>,
            )>("committee_selection_beacon"),
            committee_selection_beacon_non_revealing_node: atomo
                .resolve::<NodeIndex, ()>("committee_selection_beacon_non_revealing_node"),
            withdraws: atomo.resolve::<u64, WithdrawInfo>("withdraws"),
            assigned_jobs: atomo.resolve::<NodeIndex, Vec<[u8; 32]>>("assigned_jobs"),
            jobs: atomo.resolve::<[u8; 32], Job>("jobs"),
            mints: atomo.resolve::<[u8; 32], MintInfo>("mints"),
            inner: atomo,
        }
    }

    fn atomo_from_checkpoint(
        path: impl AsRef<Path>,
        hash: [u8; 32],
        checkpoint: &[u8],
    ) -> anyhow::Result<Atomo<QueryPerm, Self::Backend>> {
        let state_tree_tables = ApplicationStateTree::state_tree_tables();
        let backend = AtomoStorageBuilder::new(Some(path.as_ref()))
            .from_checkpoint(hash, checkpoint, &state_tree_tables)
            .read_only();

        let atomo = ApplicationState::register_tables(AtomoBuilder::<
            AtomoStorageBuilder,
            DefaultSerdeBackend,
        >::new(backend))
        .build()?
        .query();

        Ok(atomo)
    }

    fn atomo_from_path(path: impl AsRef<Path>) -> anyhow::Result<Atomo<QueryPerm, Self::Backend>> {
        let backend = AtomoStorageBuilder::new(Some(path.as_ref())).read_only();

        let atomo = ApplicationState::register_tables(AtomoBuilder::<
            AtomoStorageBuilder,
            DefaultSerdeBackend,
        >::new(backend))
        .build()?
        .query();

        Ok(atomo)
    }

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

    #[inline]
    fn client_key_to_account_key(&self, pub_key: &ClientPublicKey) -> Option<EthAddress> {
        self.inner.run(|ctx| {
            self.client_table
                .get(ctx)
                .get(pub_key)
                .map(|(address, _)| address)
        })
    }

    #[inline]
    fn get_client_nonce(&self, pub_key: &ClientPublicKey) -> Nonce {
        self.inner
            .run(|ctx| {
                self.client_table
                    .get(ctx)
                    .get(pub_key)
                    .map(|(_, nonce)| nonce)
            })
            .unwrap_or_default()
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

    #[inline]
    fn get_node_table_iter<V>(&self, closure: impl FnOnce(KeyIterator<NodeIndex>) -> V) -> V {
        self.inner
            .run(|ctx| closure(self.node_table.get(ctx).keys()))
    }

    fn pubkey_to_index(&self, pub_key: &NodePublicKey) -> Option<NodeIndex> {
        self.inner
            .run(|ctx| self.pub_key_to_index.get(ctx).get(pub_key))
    }

    #[inline]
    fn get_committee_info<V>(
        &self,
        epoch: &Epoch,
        selector: impl FnOnce(Committee) -> V,
    ) -> Option<V> {
        self.inner
            .run(|ctx| self.committee_table.get(ctx).get(epoch))
            .map(selector)
    }

    fn get_committee_selection_beacons(
        &self,
    ) -> FxHashMap<
        NodeIndex,
        (
            CommitteeSelectionBeaconCommit,
            Option<CommitteeSelectionBeaconReveal>,
        ),
    > {
        self.inner
            .run(|ctx| self.committee_selection_beacon.get(ctx).as_map())
    }

    fn get_committee_selection_beacon(
        &self,
        node: &NodeIndex,
    ) -> Option<(
        CommitteeSelectionBeaconCommit,
        Option<CommitteeSelectionBeaconReveal>,
    )> {
        self.inner
            .run(|ctx| self.committee_selection_beacon.get(ctx).get(node))
    }

    fn get_committee_selection_beacon_non_revealing_nodes(&self) -> Vec<NodeIndex> {
        self.inner
            .run(|ctx| {
                self.committee_selection_beacon_non_revealing_node
                    .get(ctx)
                    .as_map()
            })
            .keys()
            .cloned()
            .collect()
    }

    fn get_service_info(&self, id: &ServiceId) -> Option<Service> {
        self.inner.run(|ctx| self.services_table.get(ctx).get(id))
    }

    fn get_protocol_param(&self, param: &ProtocolParamKey) -> Option<ProtocolParamValue> {
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

    fn get_latencies(&self, nodes: &(NodeIndex, NodeIndex)) -> Option<Duration> {
        self.inner.run(|ctx| self.latencies.get(ctx).get(nodes))
    }

    fn get_latencies_iter<V>(
        &self,
        closure: impl FnOnce(KeyIterator<(NodeIndex, NodeIndex)>) -> V,
    ) -> V {
        self.inner
            .run(|ctx| closure(self.latencies.get(ctx).keys()))
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
            let app = ApplicationState::executor(ctx);
            app.execute_transaction(txn)
        })
    }

    fn get_node_uptime(&self, node_index: &NodeIndex) -> Option<u8> {
        self.inner
            .run(|ctx| self.uptime_table.get(ctx).get(node_index))
    }

    fn get_uri_providers(&self, uri: &Blake3Hash) -> Option<BTreeSet<NodeIndex>> {
        self.inner.run(|ctx| self.uri_to_node.get(ctx).get(uri))
    }

    fn get_content_registry(&self, node_index: &NodeIndex) -> Option<BTreeSet<Blake3Hash>> {
        self.inner
            .run(|ctx| self.node_to_uri.get(ctx).get(node_index))
    }

    /// Returns the state tree root hash from the application state.
    fn get_state_root(&self) -> Result<StateRootHash> {
        self.run(|ctx| ApplicationStateTree::get_state_root(ctx))
            .map_err(From::from)
    }

    /// Returns the state proof for a given key from the application state using the state tree.
    fn get_state_proof(
        &self,
        key: StateProofKey,
    ) -> Result<(
        Option<StateProofValue>,
        <ApplicationStateTree as StateTree>::Proof,
    )> {
        type Serde = <ApplicationStateTree as StateTree>::Serde;

        self.run(|ctx| {
            let (table, serialized_key) = key.raw::<Serde>();
            let proof = ApplicationStateTree::get_state_proof(ctx, &table, serialized_key.clone())?;
            let value = self
                .run(|ctx| ctx.get_raw_value(table, &serialized_key))
                .map(|value| key.value::<Serde>(value));
            Ok((value, proof))
        })
    }

    /// Wait for genesis block to be applied.
    ///
    /// Returns true, and immediately, if the genesis block was already applied.
    async fn wait_for_genesis(&self) -> bool {
        if !self.has_genesis() {
            tracing::warn!(
                "Genesis does not exist in application state, waiting for genesis block to be applied..."
            );

            let mut poll = tokio::time::interval(Duration::from_millis(100));

            while !self.has_genesis() {
                poll.tick().await;
            }
        }

        true
    }

    /// Returns whether the genesis block has been applied.
    fn has_genesis(&self) -> bool {
        // This is consistent with the logic in `Env::apply_genesis_block`.
        self.get_metadata(&Metadata::Epoch).is_some()
    }

    fn get_withdraws(&self, paging: WithdrawPagingParams) -> Vec<WithdrawInfoWithId> {
        self.inner
            .run(|ctx| self.withdraws.get(ctx).as_map())
            .into_iter()
            .map(|(id, info)| WithdrawInfoWithId { id, info })
            .filter(|info| info.id >= paging.start)
            .take(paging.limit)
            .collect()
    }

    fn has_minted(&self, tx_hash: [u8; 32]) -> bool {
        self.inner
            .run(|ctx| self.mints.get(ctx).get(tx_hash).is_some())
    }

    fn get_job_assignments(&self) -> Vec<(NodeIndex, Vec<[u8; 32]>)> {
        self.inner
            .run(|ctx| self.assigned_jobs.get(ctx).as_map())
            .into_iter()
            .collect()
    }

    fn get_jobs_for_node(&self, node_index: &NodeIndex) -> Option<Vec<Job>> {
        self.inner.run(|ctx| {
            let mut jobs = Vec::new();
            for job_hash in self.assigned_jobs.get(ctx).get(node_index)? {
                match self.jobs.get(ctx).get(job_hash) {
                    Some(job) => jobs.push(job),
                    None => tracing::warn!("there was no job found for job id `{job_hash:?}`"),
                }
            }
            Some(jobs)
        })
    }

    fn get_all_jobs(&self) -> Vec<Job> {
        self.inner
            .run(|ctx| self.jobs.get(ctx).as_map())
            .into_values()
            .collect()
    }
}
