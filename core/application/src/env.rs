use std::collections::BTreeSet;
use std::path::Path;
use std::time::{Duration, SystemTime};

use affair::AsyncWorker as WorkerTrait;
use anyhow::{Context, Result};
use atomo::{Atomo, AtomoBuilder, DefaultSerdeBackend, QueryPerm, UpdatePerm};
use atomo_rocks::{Cache as RocksCache, Env as RocksEnv, Options};
use fleek_crypto::{ClientPublicKey, ConsensusPublicKey, EthAddress, NodePublicKey};
use hp_fixed::unsigned::HpUfixed;
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::types::{
    AccountInfo,
    Blake3Hash,
    Block,
    BlockExecutionResponse,
    Committee,
    CommodityTypes,
    CompressionAlgorithm,
    Epoch,
    ExecutionData,
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
    TransactionReceipt,
    TransactionResponse,
    TxHash,
    Value,
};
use lightning_interfaces::{BlockStoreInterface, IncrementalPutInterface};
use lightning_utils::application::QueryRunnerExt;
use tracing::{info, warn};

use crate::config::{Config, Mode, StorageConfig};
use crate::genesis::{Genesis, GenesisPrices};
use crate::query_runner::QueryRunner;
use crate::state::State;
use crate::storage::{AtomoStorage, AtomoStorageBuilder};
use crate::table::StateTables;

pub struct Env<P> {
    inner: Atomo<P, AtomoStorage>,
}

impl Env<UpdatePerm> {
    pub fn new(config: &Config, checkpoint: Option<([u8; 32], &[u8])>) -> Result<Self> {
        let storage = match config.storage {
            StorageConfig::RocksDb => {
                let db_path = config
                    .db_path
                    .as_ref()
                    .context("db_path must be specified for RocksDb backend")?;
                let mut db_options = if let Some(db_options) = config.db_options.as_ref() {
                    let (options, _) = Options::load_latest(
                        db_options,
                        RocksEnv::new().context("Failed to create rocks db env.")?,
                        false,
                        // TODO(matthias): I set this lru cache size arbitrarily
                        RocksCache::new_lru_cache(100),
                    )
                    .context("Failed to create rocks db options.")?;
                    options
                } else {
                    Options::default()
                };
                db_options.create_if_missing(true);
                db_options.create_missing_column_families(true);
                match checkpoint {
                    Some((hash, checkpoint)) => AtomoStorageBuilder::new(Some(db_path.as_path()))
                        .with_options(db_options)
                        .from_checkpoint(hash, checkpoint),
                    None => {
                        AtomoStorageBuilder::new(Some(db_path.as_path())).with_options(db_options)
                    },
                }
            },
            StorageConfig::InMemory => AtomoStorageBuilder::new::<&Path>(None),
        };

        let mut atomo = AtomoBuilder::<AtomoStorageBuilder, DefaultSerdeBackend>::new(storage);
        atomo = atomo
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
            .with_table::<Blake3Hash, BTreeSet<NodeIndex>>("cid_to_node")
            .with_table::<NodeIndex, BTreeSet<Blake3Hash>>("node_to_cid")
            .with_table::<Blake3Hash, u32>("js_hash_count")
            .enable_iter("current_epoch_served")
            .enable_iter("rep_measurements")
            .enable_iter("submitted_rep_measurements")
            .enable_iter("rep_scores")
            .enable_iter("latencies")
            .enable_iter("node")
            .enable_iter("executed_digests")
            .enable_iter("uptime")
            .enable_iter("service_revenue")
            .enable_iter("cid_to_node")
            .enable_iter("node_to_cid")
            .enable_iter("js_hash_count");

        #[cfg(debug_assertions)]
        {
            atomo = atomo
                .enable_iter("consensus_key_to_index")
                .enable_iter("pub_key_to_index");
        }

        Ok(Self {
            inner: atomo.build()?,
        })
    }

    #[autometrics::autometrics]
    async fn run<F, P>(&mut self, mut block: Block, get_putter: F) -> BlockExecutionResponse
    where
        F: FnOnce() -> P,
        P: IncrementalPutInterface,
    {
        let response = self.inner.run(move |ctx| {
            // Create the app/execution environment
            let backend = StateTables {
                table_selector: ctx,
            };
            let app = State::new(backend);
            let last_block_hash = app.get_block_hash();
            // increment the block_number
            let block_number = app.increment_block_number();

            // Create block response
            let mut response = BlockExecutionResponse {
                block_hash: block.digest,
                parent_hash: last_block_hash,
                change_epoch: false,
                node_registry_delta: Vec::new(),
                txn_receipts: Vec::with_capacity(block.transactions.len()),
                block_number,
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

                let receipt = TransactionReceipt {
                    block_hash: block.digest,
                    block_number,
                    transaction_index: index as u64,
                    transaction_hash: txn.hash(),
                    from: txn.sender(),
                    to: txn.to(),
                    response: results,
                };
                /* Todo(dalton): Check if the transaction resulted in the committee change(Like a current validator getting slashed)
                    if so acknowledge that in the block response
                */
                response.txn_receipts.push(receipt);
            }
            // Set the last executed block hash
            app.set_last_block(block.digest);

            // Return the response
            response
        });

        if response.change_epoch {
            let epoch = self.query_runner().get_epoch_info().epoch;
            info!("#######################################");
            info!("Epoch changed to {epoch}");
            info!("#######################################");
            let storage = self.inner.get_storage_backend_unsafe();
            // This will return `None` only if the InMemory backend is used.
            if let Some(checkpoint) = storage.serialize() {
                let mut blockstore_put = get_putter();
                if blockstore_put
                    .write(checkpoint.as_slice(), CompressionAlgorithm::Uncompressed)
                    .is_ok()
                {
                    if let Ok(state_hash) = blockstore_put.finalize().await {
                        // Only temporary: write the checkpoint to disk directly.
                        self.update_last_epoch_hash(state_hash);
                    } else {
                        warn!("Failed to finalize writing checkpoint to blockstore");
                    }
                } else {
                    warn!("Failed to write checkpoint to blockstore");
                }
            }
        }

        response
    }

    /// Returns an identical environment but with query permissions
    pub fn query_socket(&self) -> Env<QueryPerm> {
        Env {
            inner: self.inner.query(),
        }
    }

    pub fn query_runner(&self) -> QueryRunner {
        QueryRunner::init(self.inner.query())
    }

    /// Tries to seeds the application state with the genesis block
    /// This function will panic if the genesis file cannot be decoded into the correct types
    /// Will return true if database was empty and genesis needed to be loaded or false if there was
    /// already state loaded and it didn't load genesis
    pub fn genesis(&mut self, config: &Config) -> bool {
        self.inner.run(|ctx| {
            let mut metadata_table = ctx.get_table::<Metadata, Value>("metadata");

            if metadata_table.get(Metadata::Epoch).is_some() {
                return false;
            }
            let mut genesis = Genesis::load().unwrap();

            match &config.mode {
                Mode::Dev => {
                    genesis.epoch_start = SystemTime::now()
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .unwrap()
                        .as_millis() as u64
                },
                Mode::Test => {
                    if let Some(config_genesis) = &config.genesis {
                        genesis = config_genesis.clone();
                    }
                },
                Mode::Prod => (),
            }

            let mut node_table = ctx.get_table::<NodeIndex, NodeInfo>("node");
            let mut account_table = ctx.get_table::<EthAddress, AccountInfo>("account");
            let mut client_table = ctx.get_table::<ClientPublicKey, EthAddress>("client_keys");
            let mut service_table = ctx.get_table::<ServiceId, Service>("service");
            let mut param_table = ctx.get_table::<ProtocolParams, u128>("parameter");
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
            param_table.insert(ProtocolParams::MaxBoost, genesis.max_boost as u128);
            param_table.insert(ProtocolParams::MaxStakeLockTime, genesis.max_lock_time as u128);
            param_table.insert(ProtocolParams::EpochTime, genesis.epoch_time as u128);
            param_table.insert(ProtocolParams::MinimumNodeStake, genesis.min_stake as u128);
            param_table.insert(ProtocolParams::LockTime, genesis.lock_time as u128);
            param_table.insert(ProtocolParams::MaxInflation, genesis.max_inflation as u128);
            param_table.insert(ProtocolParams::NodeShare, genesis.node_share as u128);
            param_table.insert(
                ProtocolParams::ProtocolShare,
                genesis.protocol_share as u128,
            );
            param_table.insert(
                ProtocolParams::ServiceBuilderShare,
                genesis.service_builder_share as u128,
            );
            param_table.insert(
                ProtocolParams::EligibilityTime,
                genesis.eligibility_time as u128,
            );
            param_table.insert(
                ProtocolParams::CommitteeSize,
                genesis.committee_size as u128,
            );

            let epoch_end: u64 = genesis.epoch_time + genesis.epoch_start;
            let mut committee_members = Vec::with_capacity(4);

            // add node info
            for node in genesis.node_info {
                let mut node_info = NodeInfo::from(&node);
                node_info.stake.staked = genesis.min_stake.into();
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
                    current_epoch_served_table.insert(node_index, served);
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

            }
            metadata_table.insert(Metadata::GenesisCommittee,
                 Value::GenesisCommittee(committee_members.clone()));
            committee_table.insert(
                0,
                Committee {
                    ready_to_change: Vec::with_capacity(committee_members.len()),
                    members: committee_members,
                    epoch_end_timestamp: epoch_end,
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
                total_served_table.insert(epoch, total_served);
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
        true
        })
    }

    // Should only be called after saving or loading from an epoch checkpoint
    pub fn update_last_epoch_hash(&mut self, state_hash: [u8; 32]) {
        self.inner.run(move |ctx| {
            let backend = StateTables {
                table_selector: ctx,
            };
            let app = State::new(backend);
            app.set_last_epoch_hash(state_hash);
        })
    }
}

impl Default for Env<UpdatePerm> {
    fn default() -> Self {
        Self::new(&Config::default(), None).unwrap()
    }
}

/// The socket that receives all update transactions
pub struct UpdateWorker<C: Collection> {
    env: Env<UpdatePerm>,
    blockstore: C::BlockStoreInterface,
}

impl<C: Collection> UpdateWorker<C> {
    pub fn new(env: Env<UpdatePerm>, blockstore: C::BlockStoreInterface) -> Self {
        Self { env, blockstore }
    }
}

impl<C: Collection> WorkerTrait for UpdateWorker<C> {
    type Request = Block;
    type Response = BlockExecutionResponse;
    async fn handle(&mut self, req: Self::Request) -> Self::Response {
        self.env.run(req, || self.blockstore.put(None)).await
    }
}
