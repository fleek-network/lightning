use std::collections::BTreeSet;
use std::time::Duration;

use anyhow::{anyhow, Result};
use atomo::{
    Atomo,
    AtomoBuilder,
    DefaultSerdeBackend,
    StorageBackendConstructor,
    TableSelector,
    UpdatePerm,
};
use fleek_crypto::{ClientPublicKey, ConsensusPublicKey, EthAddress, NodePublicKey};
use hp_fixed::unsigned::HpUfixed;
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
    TxHash,
    Value,
};
use lightning_interfaces::SyncQueryRunnerInterface;
use merklize::hashers::keccak::KeccakHasher;
use merklize::providers::mpt::MptMerklizeProvider;
use merklize::MerklizeProvider;

use super::context::StateContext;
use super::executor::StateExecutor;
use super::query::QueryRunner;
use crate::storage::AtomoStorage;

/// A canonical application state tree implementation.
pub type ApplicationMerklizeProvider =
    MptMerklizeProvider<AtomoStorage, DefaultSerdeBackend, KeccakHasher>;

/// The shared application state accumulates by executing transactions.
pub struct ApplicationState<StateTree: MerklizeProvider> {
    db: Atomo<UpdatePerm, StateTree::Storage, StateTree::Serde>,
}

impl<StateTree> ApplicationState<StateTree>
where
    StateTree: MerklizeProvider<Storage = AtomoStorage, Serde = DefaultSerdeBackend>,
{
    /// Creates a new application state.
    pub(crate) fn new(db: Atomo<UpdatePerm, AtomoStorage, DefaultSerdeBackend>) -> Self {
        Self { db }
    }

    /// Registers the application and state tree tables, and builds the atomo database.
    pub fn build<C>(atomo: AtomoBuilder<C, DefaultSerdeBackend>) -> Result<Self>
    where
        C: StorageBackendConstructor<Storage = AtomoStorage>,
    {
        let atomo = ApplicationState::<StateTree>::register_tables(atomo);

        let db = atomo
            .build()
            .map_err(|e| anyhow!("Failed to build atomo: {:?}", e))?;

        Ok(Self::new(db))
    }

    /// Returns a reader for the application state.
    pub fn query(&self) -> QueryRunner {
        QueryRunner::new(self.db.query())
    }

    /// Returns a mutable reference to the atomo storage backend.
    ///
    /// This is unsafe because it allows modifying the state tree without going through the
    /// executor, which can lead to inconsistent state across nodes.
    pub fn get_storage_backend_unsafe(&mut self) -> &AtomoStorage {
        self.db.get_storage_backend_unsafe()
    }

    /// Returns a state executor that handles transaction execution logic, reading and modifying the
    /// state.
    pub fn executor(
        ctx: &mut TableSelector<AtomoStorage, DefaultSerdeBackend>,
    ) -> StateExecutor<StateContext<AtomoStorage, DefaultSerdeBackend>> {
        StateExecutor::new(StateContext {
            table_selector: ctx,
        })
    }

    /// Runs a mutation on the state.
    pub fn run<F, R>(&mut self, mutation: F) -> Result<R>
    where
        F: FnOnce(&mut TableSelector<AtomoStorage, DefaultSerdeBackend>) -> R,
    {
        self.db.run(|ctx| {
            let result = mutation(ctx);

            StateTree::update_state_tree_from_context(ctx)?;

            Ok(result)
        })
    }

    /// Registers and configures the application state tables with the atomo database builder.
    pub fn register_tables<B: StorageBackendConstructor>(
        builder: AtomoBuilder<B, StateTree::Serde>,
    ) -> AtomoBuilder<B, StateTree::Serde> {
        let mut builder = builder
            .with_table::<Metadata, Value>("metadata")
            .with_table::<EthAddress, AccountInfo>("account")
            .with_table::<ClientPublicKey, EthAddress>("client_keys")
            .with_table::<NodeIndex, NodeInfo>("node")
            .with_table::<ConsensusPublicKey, NodeIndex>("consensus_key_to_index")
            .with_table::<NodePublicKey, NodeIndex>("pub_key_to_index")
            .with_table::<(NodeIndex, NodeIndex), Duration>("latencies")
            .with_table::<Epoch, Committee>("committee")
            .with_table::<ServiceId, Service>("service")
            .with_table::<ProtocolParams, u128>("parameter")
            .with_table::<NodeIndex, Vec<ReportedReputationMeasurements>>("rep_measurements")
            .with_table::<NodeIndex, u8>("rep_scores")
            .with_table::<NodeIndex, u8>("submitted_rep_measurements")
            .with_table::<NodeIndex, NodeServed>("current_epoch_served")
            .with_table::<NodeIndex, NodeServed>("last_epoch_served")
            .with_table::<Epoch, TotalServed>("total_served")
            .with_table::<CommodityTypes, HpUfixed<6>>("commodity_prices")
            .with_table::<ServiceId, ServiceRevenue>("service_revenue")
            .with_table::<TxHash, ()>("executed_digests")
            .with_table::<NodeIndex, u8>("uptime")
            .with_table::<Blake3Hash, BTreeSet<NodeIndex>>("uri_to_node")
            .with_table::<NodeIndex, BTreeSet<Blake3Hash>>("node_to_uri")
            .enable_iter("current_epoch_served")
            .enable_iter("rep_measurements")
            .enable_iter("submitted_rep_measurements")
            .enable_iter("rep_scores")
            .enable_iter("latencies")
            .enable_iter("node")
            .enable_iter("executed_digests")
            .enable_iter("uptime")
            .enable_iter("service_revenue")
            .enable_iter("uri_to_node")
            .enable_iter("node_to_uri");

        #[cfg(debug_assertions)]
        {
            builder = builder
                .enable_iter("consensus_key_to_index")
                .enable_iter("pub_key_to_index");
        }

        builder = StateTree::register_tables(builder);

        builder
    }
}
