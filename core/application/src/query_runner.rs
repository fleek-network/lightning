use std::collections::BTreeSet;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use atomo::{Atomo, KeyIterator, QueryPerm, ResolvedTableReference};
use fleek_crypto::{ClientPublicKey, EthAddress, NodePublicKey};
use hp_fixed::unsigned::HpUfixed;
use lightning_interfaces::application::SyncQueryRunnerInterface;
use lightning_interfaces::types::{
    AccountInfo,
    Blake3Hash,
    Committee,
    CommodityTypes,
    Epoch,
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
    TransactionRequest,
    TransactionResponse,
    TxHash,
    Value,
};
use tracing::error;

use crate::state::State;
use crate::storage::AtomoStorage;
use crate::table::StateTables;

// Define a global atomic counter
static QUERY_RUNNER_COUNT: AtomicUsize = AtomicUsize::new(0);

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
    js_hash_count: ResolvedTableReference<Blake3Hash, u32>,
    file: &'static str,
    line: u32,
}

impl QueryRunner {
    pub fn init(atomo: Atomo<QueryPerm, AtomoStorage>) -> Self {
        QUERY_RUNNER_COUNT.fetch_add(1, Ordering::Relaxed);
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
            js_hash_count: atomo.resolve::<Blake3Hash, u32>("js_hash_count"),
            file: "initial",
            line: 0,
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

    fn client_key_to_account_key(&self, pub_key: &ClientPublicKey) -> Option<EthAddress> {
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

    fn get_js_hashes(&self) -> Vec<(Blake3Hash, u32)> {
        self.inner.run(|ctx| {
            let keys = self.js_hash_count.get(ctx).keys();
            let mut hashes = Vec::new();
            for key in keys {
                if let Some(count) = self.js_hash_count.get(ctx).get(key) {
                    hashes.push((key, count));
                }
            }
            hashes
        })
    }

    fn get_js_hashes_count(&self) -> u128 {
        self.inner.run(|ctx| {
            let keys = self.js_hash_count.get(ctx).keys();
            let mut count_all: u128 = 0;
            for key in keys {
                if let Some(count) = self.js_hash_count.get(ctx).get(key) {
                    count_all = count_all.saturating_add(count as u128);
                }
            }
            count_all
        })
    }

    fn my_clone(&self, file: &'static str, line: u32) -> Self {
        let count = QUERY_RUNNER_COUNT.fetch_add(1, Ordering::Relaxed);

        error!(
            "Cloning a query runner in file {file} at line {line}. There are now {} query runners",
            count + 1
        );

        Self {
            inner: self.inner.clone(),
            metadata_table: self.metadata_table.clone(),
            account_table: self.account_table.clone(),
            client_table: self.client_table.clone(),
            node_table: self.node_table.clone(),
            pub_key_to_index: self.pub_key_to_index.clone(),
            committee_table: self.committee_table.clone(),
            services_table: self.services_table.clone(),
            param_table: self.param_table.clone(),
            current_epoch_served: self.current_epoch_served.clone(),
            rep_measurements: self.rep_measurements.clone(),
            latencies: self.latencies.clone(),
            rep_scores: self.rep_scores.clone(),
            _last_epoch_served: self._last_epoch_served.clone(),
            total_served_table: self.total_served_table.clone(),
            _service_revenue: self._service_revenue.clone(),
            _commodity_price: self._commodity_price.clone(),
            executed_digests_table: self.executed_digests_table.clone(),
            uptime_table: self.uptime_table.clone(),
            _cid_to_node: self._cid_to_node.clone(),
            _node_to_cid: self._node_to_cid.clone(),
            js_hash_count: self.js_hash_count.clone(),
            file,
            line,
        }
    }
}

impl Drop for QueryRunner {
    fn drop(&mut self) {
        let count = QUERY_RUNNER_COUNT.fetch_sub(1, Ordering::Relaxed);
        let current_time = SystemTime::now();

        let unix_time = current_time
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards!")
            .as_secs();

        error!(
            "We are dropping a query runner at {}. There are {} left. The query runner was cloned in file {} in line {}",
            unix_time,
            count - 1,
            self.file,
            self.line,
        );
    }
}

//impl Clone for QueryRunner {
//    fn clone(&self) -> Self {
//        let count = QUERY_RUNNER_COUNT.fetch_add(1, Ordering::Relaxed);
//
//        error!(
//            "Cloning a query runner. There are now {} query runners",
//            count + 1
//        );
//
//        Self {
//            inner: self.inner.clone(),
//            metadata_table: self.metadata_table.clone(),
//            account_table: self.account_table.clone(),
//            client_table: self.client_table.clone(),
//            node_table: self.node_table.clone(),
//            pub_key_to_index: self.pub_key_to_index.clone(),
//            committee_table: self.committee_table.clone(),
//            services_table: self.services_table.clone(),
//            param_table: self.param_table.clone(),
//            current_epoch_served: self.current_epoch_served.clone(),
//            rep_measurements: self.rep_measurements.clone(),
//            latencies: self.latencies.clone(),
//            rep_scores: self.rep_scores.clone(),
//            _last_epoch_served: self._last_epoch_served.clone(),
//            total_served_table: self.total_served_table.clone(),
//            _service_revenue: self._service_revenue.clone(),
//            _commodity_price: self._commodity_price.clone(),
//            executed_digests_table: self.executed_digests_table.clone(),
//            uptime_table: self.uptime_table.clone(),
//            _cid_to_node: self._cid_to_node.clone(),
//            _node_to_cid: self._node_to_cid.clone(),
//            js_hash_count: self.js_hash_count.clone(),
//            file: file!(),
//            line,
//        }
//    }
//}
