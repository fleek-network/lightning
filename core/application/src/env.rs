use std::sync::Arc;
use std::time::Duration;

use affair::AsyncWorker as WorkerTrait;
use anyhow::{Context, Result};
use atomo::{DefaultSerdeBackend, SerdeBackend, StorageBackend};
use fleek_crypto::{ClientPublicKey, ConsensusPublicKey, EthAddress, NodePublicKey};
use hp_fixed::unsigned::HpUfixed;
use lightning_interfaces::prelude::*;
use lightning_interfaces::types::{
    AccountInfo,
    Block,
    BlockExecutionResponse,
    Committee,
    CommodityTypes,
    CompressionAlgorithm,
    Epoch,
    ExecutionData,
    Genesis,
    GenesisPrices,
    Metadata,
    NodeIndex,
    NodeInfo,
    NodeServed,
    ProtocolParamKey,
    ProtocolParamValue,
    Service,
    ServiceId,
    TotalServed,
    TransactionReceipt,
    TransactionResponse,
    Value,
};
use lightning_metrics::increment_counter;
use merklize::hashers::keccak::KeccakHasher;
use merklize::trees::mpt::MptStateTree;
use merklize::StateTree;
use tokio::sync::Mutex;
use tracing::warn;
use types::{NodeRegistryChange, NodeRegistryChanges};

use crate::config::ApplicationConfig;
use crate::state::{ApplicationState, QueryRunner};
use crate::storage::AtomoStorage;

pub struct Env<B: StorageBackend, S: SerdeBackend, T: StateTree> {
    pub inner: ApplicationState<B, S, T>,
}

/// The canonical application state tree provider.
pub type ApplicationStateTree = MptStateTree<AtomoStorage, DefaultSerdeBackend, KeccakHasher>;

/// The canonical application environment.
pub type ApplicationEnv = Env<AtomoStorage, DefaultSerdeBackend, ApplicationStateTree>;

impl ApplicationEnv {
    pub fn new(config: &ApplicationConfig, checkpoint: Option<([u8; 32], &[u8])>) -> Result<Self> {
        let state_tree_tables = ApplicationStateTree::state_tree_tables();
        let builder = config.atomo_builder(
            checkpoint.map(|(hash, checkpoint)| (hash, checkpoint, state_tree_tables.as_slice())),
        )?;

        Ok(Self {
            inner: ApplicationState::build(builder)?,
        })
    }

    pub fn query_runner(&self) -> QueryRunner {
        self.inner.query()
    }

    async fn run<F, P>(&mut self, mut block: Block, get_putter: F) -> Result<BlockExecutionResponse>
    where
        F: FnOnce() -> P,
        P: IncrementalPutInterface,
    {
        let response = self
            .inner
            .run(move |ctx| {
                // Create the app/execution environment
                let state_root =
                    ApplicationStateTree::get_state_root(ctx).map_err(|e| anyhow::anyhow!(e))?;
                let app = ApplicationState::executor(ctx);
                let last_block_hash = app.get_block_hash();

                let last_block_number = app.get_block_number();
                let block_number = last_block_number + 1;

                let committee_before_execution = app.get_current_committee();

                // Create block response
                let mut response = BlockExecutionResponse {
                    block_hash: block.digest,
                    parent_hash: last_block_hash,
                    change_epoch: false,
                    node_registry_changes: Default::default(),
                    txn_receipts: Vec::with_capacity(block.transactions.len()),
                    block_number,
                    previous_state_root: state_root.into(),
                    new_state_root: [0u8; 32],
                };

                // Execute each transaction and add the results to the block response
                for (index, txn) in &mut block.transactions.iter_mut().enumerate() {
                    let results = match app.verify_transaction(txn) {
                        Ok(_) => app.execute_transaction(txn.clone()),
                        Err(err) => TransactionResponse::Revert(err),
                    };

                    // If the transaction moved the epoch forward, acknowledge that in the block
                    // response
                    if let TransactionResponse::Success(ExecutionData::EpochChange) = results {
                        response.change_epoch = true;
                    }

                    let mut event = None;
                    if let TransactionResponse::Success(_) = results {
                        if let Some(e) = txn.event() {
                            event = Some(e);
                        }
                    }

                    let receipt = TransactionReceipt {
                        block_hash: block.digest,
                        block_number,
                        transaction_index: index as u64,
                        transaction_hash: txn.hash(),
                        from: txn.sender(),
                        to: txn.to(),
                        response: results,
                        event,
                    };
                    /* Todo(dalton): Check if the transaction resulted in the committee change(Like a current validator getting slashed)
                        if so acknowledge that in the block response
                    */
                    response.txn_receipts.push(receipt);
                }

                // Update node registry changes on the response if there were any for this block.
                let committee = app.get_current_committee();
                if let Some(committee) = &committee {
                    if let Some(changes) = committee.node_registry_changes.get(&block_number) {
                        response.node_registry_changes = changes.clone();
                    }
                }

                // If the committee changed, advance to the next epoch era.
                let has_committee_members_changes =
                    committee_before_execution.map(|c| c.members) != committee.map(|c| c.members);
                if has_committee_members_changes {
                    app.set_epoch_era(app.get_epoch_era() + 1);
                }

                // Set the last executed block hash and sub dag index
                // if epoch changed a new committee starts and subdag starts back at 0
                let (new_sub_dag_index, new_sub_dag_round) =
                    if response.change_epoch || has_committee_members_changes {
                        (0, 0)
                    } else {
                        (block.sub_dag_index, block.sub_dag_round)
                    };
                app.set_last_block(response.block_hash, new_sub_dag_index, new_sub_dag_round);

                // Set the new state root on the response.
                drop(app);
                let state_root =
                    ApplicationStateTree::get_state_root(ctx).map_err(|e| anyhow::anyhow!(e))?;
                response.new_state_root = state_root.into();

                // Return the response
                Ok::<BlockExecutionResponse, anyhow::Error>(response)
            })
            .context("Failed to execute block")?
            .unwrap();

        if response.change_epoch {
            increment_counter!(
                "epoch_change_by_txn",
                Some(
                    "Counter for the number of times the node changed epochs naturally by executing transactions"
                )
            );

            // Build the checkpoint, write it to the blockstore, and update the last epoch hash in
            // the application state metadata.
            // This will return `None` only if the InMemory backend is used.
            if let Some((_, checkpoint)) = self.build_checkpoint() {
                let mut blockstore_put = get_putter();
                if blockstore_put
                    .write(checkpoint.as_slice(), CompressionAlgorithm::Uncompressed)
                    .is_ok()
                {
                    if let Ok(state_hash) = blockstore_put.finalize().await {
                        // Only temporary: write the checkpoint to disk directly.
                        self.update_last_epoch_hash(state_hash)?;
                    } else {
                        warn!("Failed to finalize writing checkpoint to blockstore");
                    }
                } else {
                    warn!("Failed to write checkpoint to blockstore");
                }
            }
        }

        Ok(response)
    }

    /// Tries to seeds the application state with the genesis block
    /// This function will panic if the genesis file cannot be decoded into the correct types
    /// Will return true if database was empty and genesis needed to be loaded or false if there was
    /// already state loaded and it didn't load genesis
    pub fn apply_genesis_block(&mut self, genesis: Genesis) -> Result<bool> {
        self.inner.run(|ctx| {
            let mut metadata_table = ctx.get_table::<Metadata, Value>("metadata");

            if metadata_table.get(Metadata::Epoch).is_some() {
                tracing::info!("Genesis block already exists in application state.");
                return Ok(false);
            }

            let mut node_table = ctx.get_table::<NodeIndex, NodeInfo>("node");
            let mut account_table = ctx.get_table::<EthAddress, AccountInfo>("account");
            let mut client_table = ctx.get_table::<ClientPublicKey, EthAddress>("client_keys");
            let mut service_table = ctx.get_table::<ServiceId, Service>("service");
            let mut param_table = ctx.get_table::<ProtocolParamKey, ProtocolParamValue>("parameter");
            let mut committee_table = ctx.get_table::<Epoch, Committee>("committee");
            let mut commodity_prices_table =
                ctx.get_table::<CommodityTypes, HpUfixed<6>>("commodity_prices");
            let mut rep_scores_table = ctx.get_table::<NodeIndex, u8>("rep_scores");
            let mut total_served_table = ctx.get_table::<Epoch, TotalServed>("total_served");
            let mut current_epoch_served_table =
                ctx.get_table::<NodeIndex, NodeServed>("current_epoch_served");
            let mut latencies_table =
                ctx.get_table::<(NodeIndex, NodeIndex), Duration>("latencies");
            let mut consensus_key_to_index_table = ctx.get_table::<ConsensusPublicKey, NodeIndex>("consensus_key_to_index");
            let mut pub_key_to_index_table = ctx.get_table::<NodePublicKey, NodeIndex>("pub_key_to_index");

            // TODO(matthias): should we hash the genesis state instead?
            metadata_table.insert(Metadata::LastEpochHash, Value::Hash([0; 32]));

            metadata_table.insert(Metadata::ChainId, Value::ChainId(genesis.chain_id));

            metadata_table.insert(Metadata::BlockNumber, Value::BlockNumber(0));

            metadata_table.insert(
                Metadata::ProtocolFundAddress,
                Value::AccountPublicKey(genesis.protocol_fund_address),
            );

            metadata_table.insert(Metadata::GovernanceAddress,
                Value::AccountPublicKey(genesis.governance_address));
            let governance_account = AccountInfo {
                flk_balance: 0u64.into(),
                stables_balance: 0u64.into(),
                bandwidth_balance: 0u64.into(),
                nonce: 0,
            };
            account_table.insert(genesis.governance_address,  governance_account);

            let supply_at_genesis: HpUfixed<18> = HpUfixed::from(genesis.supply_at_genesis);
            metadata_table.insert(
                Metadata::TotalSupply,
                Value::HpUfixed(supply_at_genesis.clone()),
            );
            metadata_table.insert(
                Metadata::SupplyYearStart,
                Value::HpUfixed(supply_at_genesis),
            );
            param_table.insert(
                ProtocolParamKey::MaxBoost,
                ProtocolParamValue::MaxBoost(genesis.max_boost)
            );
            param_table.insert(
                ProtocolParamKey::MaxStakeLockTime,
                ProtocolParamValue::MaxStakeLockTime(genesis.max_lock_time)
            );
            param_table.insert(
                ProtocolParamKey::EpochTime,
                ProtocolParamValue::EpochTime(genesis.epoch_time)
            );
            param_table.insert(
                ProtocolParamKey::EpochsPerYear,
                ProtocolParamValue::EpochsPerYear(genesis.epochs_per_year)
            );
            param_table.insert(
                ProtocolParamKey::MinimumNodeStake,
                ProtocolParamValue::MinimumNodeStake(genesis.min_stake)
            );
            param_table.insert(
                ProtocolParamKey::LockTime,
                ProtocolParamValue::LockTime(genesis.lock_time)
            );
            param_table.insert(
                ProtocolParamKey::MaxInflation,
                ProtocolParamValue::MaxInflation(genesis.max_inflation)
            );
            param_table.insert(
                ProtocolParamKey::NodeShare,
                ProtocolParamValue::NodeShare(genesis.node_share)
            );
            param_table.insert(
                ProtocolParamKey::ProtocolShare,
                ProtocolParamValue::ProtocolShare(genesis.protocol_share),
            );
            param_table.insert(
                ProtocolParamKey::ServiceBuilderShare,
                ProtocolParamValue::ServiceBuilderShare(genesis.service_builder_share),
            );
            param_table.insert(
                ProtocolParamKey::EligibilityTime,
                ProtocolParamValue::EligibilityTime(genesis.eligibility_time),
            );
            param_table.insert(
                ProtocolParamKey::CommitteeSize,
                ProtocolParamValue::CommitteeSize(genesis.committee_size),
            );
            param_table.insert(
                ProtocolParamKey::NodeCount,
                ProtocolParamValue::NodeCount(genesis.node_count)
            );
            param_table.insert(
                ProtocolParamKey::MinNumMeasurements,
                ProtocolParamValue::MinNumMeasurements(genesis.min_num_measurements)
            );
            param_table.insert(
                ProtocolParamKey::ReputationPingTimeout,
                ProtocolParamValue::ReputationPingTimeout(genesis.reputation_ping_timeout),
            );
            param_table.insert(
                ProtocolParamKey::TopologyTargetK,
                ProtocolParamValue::TopologyTargetK(genesis.topology_target_k),
            );
            param_table.insert(
                ProtocolParamKey::TopologyMinNodes,
                ProtocolParamValue::TopologyMinNodes(genesis.topology_min_nodes),
            );
            param_table.insert(
                ProtocolParamKey::CommitteeSelectionBeaconCommitPhaseDuration,
                ProtocolParamValue::CommitteeSelectionBeaconCommitPhaseDuration(
                    genesis.committee_selection_beacon_commit_phase_duration,
                ),
            );
            param_table.insert(
                ProtocolParamKey::CommitteeSelectionBeaconRevealPhaseDuration,
                ProtocolParamValue::CommitteeSelectionBeaconRevealPhaseDuration(
                    genesis.committee_selection_beacon_reveal_phase_duration,
                ),
            );
            param_table.insert(
                ProtocolParamKey::CommitteeSelectionBeaconNonRevealSlashAmount,
                ProtocolParamValue::CommitteeSelectionBeaconNonRevealSlashAmount(
                    genesis.committee_selection_beacon_non_reveal_slash_amount,
                ),
            );

            let epoch_end: u64 = genesis.epoch_time + genesis.epoch_start;
            let mut committee_members = Vec::with_capacity(4);
            let mut active_nodes = Vec::with_capacity(genesis.node_info.len());
            let mut node_registry_changes = NodeRegistryChanges::from_iter([(0, vec![])]);
            // add node info
            for node in genesis.node_info {
                let mut node_info = NodeInfo::from(&node);

                node_info.stake.staked = std::cmp::max(
                    node_info.stake.staked, genesis.min_stake.into());

                // Add the node to the node registry changes for the genesis committee.
                node_registry_changes.get_mut(&0).unwrap().push((
                    node_info.public_key,
                    NodeRegistryChange::New,
                ));

                let node_index = match metadata_table.get(Metadata::NextNodeIndex) {
                    Some(Value::NextNodeIndex(index)) => index,
                    _ => 0,
                };

                consensus_key_to_index_table.insert(node_info.consensus_key, node_index);
                pub_key_to_index_table.insert(node_info.public_key, node_index);
                node_table.insert(node_index, node_info);
                metadata_table.insert(
                    Metadata::NextNodeIndex,
                    Value::NextNodeIndex(node_index + 1),
                );
                // add genesis current epoch served if there
                if let Some(served) = node.current_epoch_served {
                    current_epoch_served_table.insert(node_index, NodeServed::from(served));
                }
                // add genesis reputation if there
                if let Some(rep) = node.reputation {
                    assert!(
                        (0..=100).contains(&rep),
                        "Reputation scores must be in range [0, 100]."
                    );
                    rep_scores_table.insert(node_index, rep);
                }
                // if there a committee member push them to the committee vec and set after loop
                if node.genesis_committee{
                    committee_members.push(node_index);
                }
                active_nodes.push(node_index);
            }

            metadata_table.insert(Metadata::GenesisCommittee,
                 Value::GenesisCommittee(committee_members.clone()));
            committee_table.insert(
                0,
                Committee {
                    ready_to_change: Vec::with_capacity(committee_members.len()),
                    members: committee_members.clone(),
                    epoch_end_timestamp: epoch_end,
                    // Todo(dont just use the committee members for first set)
                    active_node_set: active_nodes,
                    node_registry_changes,
                },
            );

            for service in &genesis.service {
                service_table.insert(
                    service.id,
                    Service {
                        owner: service.owner,
                        commodity_type: service.commodity_type,
                        slashing: (),
                    },
                )
            }

            for account in genesis.account {
                let info = AccountInfo {
                    flk_balance: account.flk_balance,
                    stables_balance: account.stables_balance.into(),
                    bandwidth_balance: account.bandwidth_balance.into(),
                    nonce: 0,
                };
                account_table.insert(account.public_key, info);
            }

            for (client_key, address) in genesis.client {
                client_table.insert(client_key, address);
            }

            // add commodity prices
            for commodity_price in genesis.commodity_prices {
                let GenesisPrices { commodity, price } = commodity_price;
                let big_price: HpUfixed<6> = price.into();
                commodity_prices_table.insert(commodity, big_price);
            }

            // add total served
            for (epoch, total_served) in genesis.total_served {
                total_served_table.insert(epoch, TotalServed::from(total_served));
            }

            // add latencies
            if let Some(latencies) = genesis.latencies {
                for lat in latencies {
                    assert!(lat.node_public_key_lhs < lat.node_public_key_rhs,
                        "Invalid latency entry, node_public_key_lhs must be smaller than node_public_key_rhs");
                    let index_lhs = pub_key_to_index_table.get(lat.node_public_key_lhs)
                        .expect("Invalid latency entry, node doesn't have an index.");
                    let index_rhs = pub_key_to_index_table.get(lat.node_public_key_rhs)
                        .expect("Invalid latency entry, node doesn't have an index.");
                    latencies_table.insert(
                        (index_lhs, index_rhs),
                        Duration::from_millis(lat.latency_in_millis),
                    );
                }
            }

            metadata_table.insert(Metadata::Epoch, Value::Epoch(0));
            metadata_table.insert(Metadata::EpochEra, Value::EpochEra(0));

            tracing::info!("Genesis block loaded into application state.");
            Ok(true)
        })?
    }

    // Should only be called after saving or loading from an epoch checkpoint
    pub fn update_last_epoch_hash(&mut self, state_hash: [u8; 32]) -> Result<()> {
        self.inner
            .run(move |ctx| {
                let app = ApplicationState::executor(ctx);
                app.set_last_epoch_hash(state_hash);
            })
            .context("Failed to update last epoch hash")
    }

    /// Build and return a new checkpoint of the application state.
    ///
    /// This method requires a mutable reference to the application state, since it
    /// serializes the state using the mutable storage backend.
    pub fn build_checkpoint(&mut self) -> Option<([u8; 32], Vec<u8>)> {
        let storage = self.inner.get_storage_backend_unsafe();
        let exclude_tables = ApplicationStateTree::state_tree_tables();
        // This will return `None` only if the InMemory backend is used.
        let checkpoint = storage.serialize(&exclude_tables)?;
        let checkpoint_hash = fleek_blake3::hash(&checkpoint);

        Some((checkpoint_hash.into(), checkpoint))
    }
}

impl Default for ApplicationEnv {
    fn default() -> Self {
        Self::new(&ApplicationConfig::default(), None).unwrap()
    }
}

/// The socket that receives all update transactions
pub struct UpdateWorker<C: NodeComponents> {
    env: Arc<Mutex<ApplicationEnv>>,
    blockstore: C::BlockstoreInterface,
}

impl<C: NodeComponents> UpdateWorker<C> {
    pub fn new(env: Arc<Mutex<ApplicationEnv>>, blockstore: C::BlockstoreInterface) -> Self {
        Self { env, blockstore }
    }
}

impl<C: NodeComponents> WorkerTrait for UpdateWorker<C> {
    type Request = Block;
    type Response = BlockExecutionResponse;

    async fn handle(&mut self, req: Self::Request) -> Self::Response {
        self.env
            .lock()
            .await
            .run(req, || self.blockstore.put(None))
            .await
            .expect("Failed to execute block")
    }
}
