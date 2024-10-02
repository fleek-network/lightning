use std::collections::BTreeSet;
use std::marker::PhantomData;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use atomo::{
    Atomo,
    AtomoBuilder,
    DefaultSerdeBackend,
    SerdeBackend,
    StorageBackend,
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
    CommitteeSelectionBeaconCommit,
    CommitteeSelectionBeaconReveal,
    CommodityTypes,
    Epoch,
    Metadata,
    NodeIndex,
    NodeInfo,
    NodeServed,
    ProtocolParamKey,
    ProtocolParamValue,
    ReportedReputationMeasurements,
    Service,
    ServiceId,
    ServiceRevenue,
    TotalServed,
    TxHash,
    Value,
};
use lightning_interfaces::SyncQueryRunnerInterface;
use merklize::StateTree;
use tracing::info;

use super::context::StateContext;
use super::executor::StateExecutor;
use super::query::QueryRunner;
use crate::env::ApplicationStateTree;
use crate::storage::AtomoStorage;

/// The application state encapsulates the atomo database and merklized state tree, and represents
/// the accumulated state over the applied history of transactions. Application state should only
/// be updated through the execution of transactions.
pub struct ApplicationState<B: StorageBackend, S: SerdeBackend, T: StateTree> {
    db: Atomo<UpdatePerm, B, S>,
    _tree: PhantomData<T>,
}

impl ApplicationState<AtomoStorage, DefaultSerdeBackend, ApplicationStateTree> {
    /// Creates a new application state.
    pub(crate) fn new(db: Atomo<UpdatePerm, AtomoStorage, DefaultSerdeBackend>) -> Self {
        Self {
            db,
            _tree: PhantomData,
        }
    }

    /// Registers the application and state tree tables, and builds the atomo database.
    pub fn build<C>(atomo: AtomoBuilder<C, DefaultSerdeBackend>) -> Result<Self>
    where
        C: StorageBackendConstructor<Storage = AtomoStorage>,
    {
        let mut atomo = Self::register_tables(atomo);

        // Register the state tree tables.
        atomo = ApplicationStateTree::register_tables(atomo);

        let mut db = atomo
            .build()
            .map_err(|e| anyhow!("Failed to build atomo: {:?}", e))?;
        let mut query = db.query();

        // If the tree is empty, rebuild/backfill it from the full state.
        if ApplicationStateTree::is_empty_state_tree_unsafe(&mut query)? {
            info!("State tree is empty, backfilling...");
            ApplicationStateTree::clear_and_rebuild_state_tree_unsafe(&mut db)?;
            let root_hash = query.run(|ctx| ApplicationStateTree::get_state_root(ctx))?;
            info!("State tree backfilled. Root hash is {root_hash}");
        }

        // Verify state tree consistency with the full state.
        ApplicationStateTree::verify_state_tree_unsafe(&mut query)?;

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

    /// Resets the state tree by clearing it and rebuilding it from the full state.
    ///
    /// This is an unsafe because it acts directly on the underlying storage backend. It should
    /// only be used in isolation (not while the node is running).
    pub fn reset_state_tree_unsafe(&mut self) -> Result<()> {
        info!("Resetting state tree...");

        ApplicationStateTree::clear_and_rebuild_state_tree_unsafe(&mut self.db)?;

        let root_hash = self
            .db
            .query()
            .run(|ctx| ApplicationStateTree::get_state_root(ctx))?;
        info!("State tree reset. Root hash is {root_hash}");

        Ok(())
    }

    /// Runs a mutation on the state.
    ///
    /// This is a wrapper around atomo's `run` that also updates the state tree based on state
    /// changes accumulated during the mutation.
    ///
    /// Returns the result of the mutation, wrapped in a `Result` that includes an error if the
    /// state tree update fails.
    pub fn run<F, R>(&mut self, mutation: F) -> Result<R>
    where
        F: FnOnce(&mut TableSelector<AtomoStorage, DefaultSerdeBackend>) -> R,
    {
        self.db.run(|ctx| {
            let result = mutation(ctx);

            // Update the state tree with the batch of changes in the current run context.
            ApplicationStateTree::update_state_tree_from_context_changes(ctx)
                .context("Failed to update state tree")?;

            Ok(result)
        })
    }

    /// Registers and configures the application state tables with the atomo database builder.
    pub fn register_tables<C: StorageBackendConstructor>(
        builder: AtomoBuilder<C, DefaultSerdeBackend>,
    ) -> AtomoBuilder<C, DefaultSerdeBackend> {
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
            .with_table::<ProtocolParamKey, ProtocolParamValue>("parameter")
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
            .with_table::<NodeIndex, (
                CommitteeSelectionBeaconCommit,
                Option<CommitteeSelectionBeaconReveal>,
            )>("committee_selection_beacon")
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
            .enable_iter("node_to_uri")
            .enable_iter("committee_selection_beacon");

        #[cfg(debug_assertions)]
        {
            builder = builder
                .enable_iter("consensus_key_to_index")
                .enable_iter("pub_key_to_index");
        }

        builder
    }
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use atomo::InMemoryStorage;
    use fleek_crypto::{AccountOwnerSecretKey, ConsensusSecretKey, NodeSecretKey, SecretKey};
    use lightning_interfaces::types::{NodePorts, Participation, Staking};

    use super::*;
    use crate::storage::AtomoStorageBuilder;

    const EMPTY_STATE_ROOT: &str =
        "bc36789e7a1e281436464229828f817d6612f7b477d66591ff96a9e064bcc98a";

    struct TestData {
        pub accounts: Vec<(EthAddress, AccountInfo)>,
        pub nodes: Vec<(NodeIndex, NodeInfo)>,
    }

    impl TestData {
        fn build(account_count: usize, node_count: usize) -> Self {
            let accounts = (0..account_count)
                .map(|_| {
                    let secret_key = AccountOwnerSecretKey::generate();
                    let public_key = secret_key.to_pk();
                    let eth_address: EthAddress = public_key.into();

                    (
                        eth_address,
                        AccountInfo {
                            flk_balance: HpUfixed::<18>::zero(),
                            stables_balance: HpUfixed::<6>::zero(),
                            bandwidth_balance: 0,
                            nonce: 0,
                        },
                    )
                })
                .collect::<Vec<_>>();
            let nodes = (0..node_count)
                .map(|node_index| {
                    let node_secret_key = NodeSecretKey::generate();
                    let node_public_key = node_secret_key.to_pk();

                    let consensus_secret_key = ConsensusSecretKey::generate();
                    let consensus_public_key = consensus_secret_key.to_pk();

                    (
                        node_index as NodeIndex,
                        NodeInfo {
                            owner: accounts[0].0,
                            public_key: node_public_key,
                            consensus_key: consensus_public_key,
                            staked_since: 0,
                            stake: Staking {
                                staked: HpUfixed::<18>::zero(),
                                stake_locked_until: 0,
                                locked: HpUfixed::<18>::zero(),
                                locked_until: 0,
                            },
                            domain: "127.0.0.1".parse().unwrap(),
                            worker_domain: "127.0.0.1".parse().unwrap(),
                            ports: NodePorts::default(),
                            worker_public_key: node_public_key,
                            participation: Participation::OptedIn,
                            nonce: 0,
                        },
                    )
                })
                .collect::<Vec<_>>();
            Self { accounts, nodes }
        }
    }

    #[test]
    fn test_state_is_updated_on_run() {
        let builder = AtomoBuilder::new(AtomoStorageBuilder::InMemory(InMemoryStorage::default()));
        let mut writer = ApplicationState::build(builder).unwrap();
        let reader = writer.query();

        // Check that the initial root hash is that of an empty state.
        let initial_root_hash = reader
            .run(|ctx| ApplicationStateTree::get_state_root(ctx))
            .unwrap();
        assert_eq!(initial_root_hash, EMPTY_STATE_ROOT);

        // Build some test data.
        let account_count = 100;
        let node_count = 100;
        let test_data = TestData::build(account_count, node_count);

        // Insert data into the state.
        writer
            .run(|ctx| {
                let mut accounts_table = ctx.get_table::<EthAddress, AccountInfo>("account");
                let mut nodes_table = ctx.get_table::<NodeIndex, NodeInfo>("node");

                for (eth_address, account) in test_data.accounts.clone() {
                    accounts_table.insert(eth_address, account);
                }

                for (node_index, node) in test_data.nodes.clone() {
                    nodes_table.insert(node_index, node);
                }
            })
            .unwrap();

        // Check that the data was inserted correctly.
        reader.run(|ctx| {
            let accounts_table = ctx.get_table::<EthAddress, AccountInfo>("account");
            let nodes_table = ctx.get_table::<NodeIndex, NodeInfo>("node");

            for (eth_address, account) in test_data.accounts.clone() {
                assert_eq!(accounts_table.get(eth_address).unwrap(), account);
            }

            for (node_index, node) in test_data.nodes.clone() {
                assert_eq!(nodes_table.get(node_index).unwrap(), node);
            }
        });

        // Check that the root hash has been updated.
        let new_root_hash = reader
            .run(|ctx| ApplicationStateTree::get_state_root(ctx))
            .unwrap();
        assert_ne!(new_root_hash, initial_root_hash);
    }

    #[test]
    fn test_state_tree_is_backfilled_on_startup_when_empty() {
        let storage = InMemoryStorage::default();

        let builder = AtomoBuilder::new(AtomoStorageBuilder::InMemory(storage.clone()));
        let mut writer = ApplicationState::build(builder).unwrap();
        let reader = writer.query();

        // Check that the initial root hash is that of an empty state.
        let initial_root_hash = reader
            .run(|ctx| ApplicationStateTree::get_state_root(ctx))
            .unwrap();
        assert_eq!(initial_root_hash, EMPTY_STATE_ROOT);

        // Build some test data.
        let account_count = 100;
        let node_count = 100;
        let test_data = TestData::build(account_count, node_count);

        // Insert data into the state.
        writer
            .run(|ctx| {
                let mut accounts_table = ctx.get_table::<EthAddress, AccountInfo>("account");
                let mut nodes_table = ctx.get_table::<NodeIndex, NodeInfo>("node");

                for (eth_address, account) in test_data.accounts.clone() {
                    accounts_table.insert(eth_address, account);
                }

                for (node_index, node) in test_data.nodes.clone() {
                    nodes_table.insert(node_index, node);
                }
            })
            .unwrap();

        // Get the root hash of the state tree with populated data.
        let populated_root_hash = reader
            .run(|ctx| ApplicationStateTree::get_state_root(ctx))
            .unwrap();

        // Clear the state tree data.
        ApplicationStateTree::clear_state_tree_unsafe(&mut writer.db).unwrap();

        // Check that the root hash is that of an empty state.
        let root_hash = reader
            .run(|ctx| ApplicationStateTree::get_state_root(ctx))
            .unwrap();
        assert_eq!(root_hash, EMPTY_STATE_ROOT);

        // Build the state writer again, where we expect the state tree to be rebuilt.
        let builder = AtomoBuilder::new(AtomoStorageBuilder::InMemory(storage));
        let writer = ApplicationState::build(builder).unwrap();
        let reader = writer.query();

        // Check that the root hash is not that of an empty state.
        let root_hash = reader
            .run(|ctx| ApplicationStateTree::get_state_root(ctx))
            .unwrap();
        assert_ne!(root_hash, EMPTY_STATE_ROOT);

        // Check that the root hash is the same as the one with populated data, confirming that the
        // state tree was rebuilt correctly.
        assert_eq!(root_hash, populated_root_hash);
    }

    #[test]
    fn test_state_tree_verified_on_startup() {
        let storage = InMemoryStorage::default();

        let builder = AtomoBuilder::new(AtomoStorageBuilder::InMemory(storage.clone()));
        let mut writer = ApplicationState::build(builder).unwrap();
        let reader = writer.query();

        // Check that the initial root hash is that of an empty state.
        let initial_root_hash = reader
            .run(|ctx| ApplicationStateTree::get_state_root(ctx))
            .unwrap();
        assert_eq!(initial_root_hash, EMPTY_STATE_ROOT);

        // Build some test data.
        let account_count = 100;
        let node_count = 100;
        let test_data = TestData::build(account_count, node_count);

        // Insert data into the state.
        writer
            .run(|ctx| {
                let mut accounts_table = ctx.get_table::<EthAddress, AccountInfo>("account");
                let mut nodes_table = ctx.get_table::<NodeIndex, NodeInfo>("node");

                for (eth_address, account) in test_data.accounts.clone() {
                    accounts_table.insert(eth_address, account);
                }

                for (node_index, node) in test_data.nodes.clone() {
                    nodes_table.insert(node_index, node);
                }
            })
            .unwrap();

        // Get the root hash of the state tree with populated data.
        let populated_root_hash = reader
            .run(|ctx| ApplicationStateTree::get_state_root(ctx))
            .unwrap();

        // Corrupt the state tree data by manually removing some state data without using the
        // state writer, so that the tree is not updated to reflect the changes.
        writer.db.run(|ctx| {
            let mut accounts_table = ctx.get_table::<EthAddress, AccountInfo>("account");
            accounts_table.remove(test_data.accounts[0].0);
        });

        // Check that the root hash has not been updated - the tree is corrupt.
        let root_hash = reader
            .run(|ctx| ApplicationStateTree::get_state_root(ctx))
            .unwrap();
        assert_eq!(root_hash, populated_root_hash);

        // Build the state writer again, and expect verification on startup to fail.
        let builder = AtomoBuilder::new(AtomoStorageBuilder::InMemory(storage));
        assert!(ApplicationState::build(builder).is_err());
    }

    #[test]
    fn test_state_backfill_on_startup_under_time_limit() {
        let storage = InMemoryStorage::default();

        let builder = AtomoBuilder::new(AtomoStorageBuilder::InMemory(storage.clone()));
        let mut writer = ApplicationState::build(builder).unwrap();
        let reader = writer.query();

        // Check that the initial root hash is that of an empty state.
        let initial_root_hash = reader
            .run(|ctx| ApplicationStateTree::get_state_root(ctx))
            .unwrap();
        assert_eq!(initial_root_hash, EMPTY_STATE_ROOT);

        // Build some test data.
        let account_count = 500;
        let node_count = 500;
        let test_data = TestData::build(account_count, node_count);

        // Insert data into the state.
        writer
            .run(|ctx| {
                let mut accounts_table = ctx.get_table::<EthAddress, AccountInfo>("account");
                let mut nodes_table = ctx.get_table::<NodeIndex, NodeInfo>("node");

                for (eth_address, account) in test_data.accounts.clone() {
                    accounts_table.insert(eth_address, account);
                }

                for (node_index, node) in test_data.nodes.clone() {
                    nodes_table.insert(node_index, node);
                }
            })
            .unwrap();

        // Get the root hash of the state tree with populated data.
        let populated_root_hash = reader
            .run(|ctx| ApplicationStateTree::get_state_root(ctx))
            .unwrap();

        // Clear the state tree data.
        ApplicationStateTree::clear_state_tree_unsafe(&mut writer.db).unwrap();

        // Get the current time.
        let start_time = Instant::now();

        // Check that the root hash is that of an empty state.
        let root_hash = reader
            .run(|ctx| ApplicationStateTree::get_state_root(ctx))
            .unwrap();
        assert_eq!(root_hash, EMPTY_STATE_ROOT);

        // Build the state writer again, where we expect the state tree to be rebuilt.
        let builder = AtomoBuilder::new(AtomoStorageBuilder::InMemory(storage));
        let writer = ApplicationState::build(builder).unwrap();
        let reader = writer.query();

        // Check that the root hash is not that of an empty state.
        let root_hash = reader
            .run(|ctx| ApplicationStateTree::get_state_root(ctx))
            .unwrap();
        assert_ne!(root_hash, EMPTY_STATE_ROOT);

        // Check that the root hash is the same as the one with populated data, confirming that the
        // state tree was rebuilt correctly.
        assert_eq!(root_hash, populated_root_hash);

        // Check that the state tree was rebuilt and verified in a reasonable time.
        let duration = start_time.elapsed();
        assert!(duration < Duration::from_secs(1));
    }

    #[test]
    fn test_state_tree_verify_on_startup_is_under_time_limit() {
        let storage = InMemoryStorage::default();

        let builder = AtomoBuilder::new(AtomoStorageBuilder::InMemory(storage.clone()));
        let mut writer = ApplicationState::build(builder).unwrap();

        // Build some test data.
        let account_count = 500;
        let node_count = 500;
        let test_data = TestData::build(account_count, node_count);

        // Insert data into the state.
        writer
            .run(|ctx| {
                let mut accounts_table = ctx.get_table::<EthAddress, AccountInfo>("account");
                let mut nodes_table = ctx.get_table::<NodeIndex, NodeInfo>("node");

                for (eth_address, account) in test_data.accounts.clone() {
                    accounts_table.insert(eth_address, account);
                }

                for (node_index, node) in test_data.nodes.clone() {
                    nodes_table.insert(node_index, node);
                }
            })
            .unwrap();

        // Get the current time.
        let start_time = Instant::now();

        // Build the state writer again, where we expect the state tree to be verified.
        let builder = AtomoBuilder::new(AtomoStorageBuilder::InMemory(storage));
        let _writer = ApplicationState::build(builder).unwrap();

        // Check that the state tree was verified on startup in a reasonable time.
        let duration = start_time.elapsed();
        assert!(duration < Duration::from_secs(1));
    }
}
