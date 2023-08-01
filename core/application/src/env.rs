use std::time::{Duration, SystemTime};

use affair::Worker as WorkerTrait;
use atomo::{Atomo, AtomoBuilder, DefaultSerdeBackend, QueryPerm, UpdatePerm};
use fleek_crypto::{AccountOwnerPublicKey, ClientPublicKey, EthAddress, NodePublicKey, PublicKey};
use hp_fixed::unsigned::HpUfixed;
use lightning_interfaces::{
    types::{
        AccountInfo, Block, CommodityTypes, Epoch, ExecutionData, Metadata, NodeInfo, NodeServed,
        ProtocolParams, ReportedReputationMeasurements, Service, ServiceId, ServiceRevenue,
        TotalServed, TransactionResponse, Value,
    },
    BlockExecutionResponse,
};

use crate::{
    config::{Config, Mode},
    genesis::{Genesis, GenesisPrices},
    query_runner::QueryRunner,
    state::{Committee, State},
    table::StateTables,
};

pub struct Env<P> {
    inner: Atomo<P>,
}

impl Env<UpdatePerm> {
    pub fn new() -> Self {
        let mut atomo = AtomoBuilder::<DefaultSerdeBackend>::new()
            .with_table::<Metadata, Value>("metadata")
            .with_table::<EthAddress, AccountInfo>("account")
            .with_table::<ClientPublicKey, EthAddress>("client_keys")
            .with_table::<NodePublicKey, NodeInfo>("node")
            .with_table::<NodePublicKey, u64>("pubkey_to_index")
            .with_table::<u64, NodePublicKey>("index_to_pubkey")
            .with_table::<(u64, u64), Duration>("latencies")
            .with_table::<Epoch, Committee>("committee")
            .with_table::<ServiceId, Service>("service")
            .with_table::<ProtocolParams, u128>("parameter")
            .with_table::<NodePublicKey, Vec<ReportedReputationMeasurements>>("rep_measurements")
            .with_table::<NodePublicKey, u8>("rep_scores")
            .with_table::<NodePublicKey, NodeServed>("current_epoch_served")
            .with_table::<NodePublicKey, NodeServed>("last_epoch_served")
            .with_table::<Epoch, TotalServed>("total_served")
            .with_table::<CommodityTypes, HpUfixed<6>>("commodity_prices")
            .with_table::<ServiceId, ServiceRevenue>("service_revenue")
            .enable_iter("current_epoch_served")
            .enable_iter("rep_measurements")
            .enable_iter("rep_scores")
            .enable_iter("latencies")
            .enable_iter("node")
            .enable_iter("service_revenue");

        #[cfg(debug_assertions)]
        {
            atomo = atomo
                .enable_iter("pubkey_to_index")
                .enable_iter("index_to_pubkey");
        }

        Self {
            inner: atomo.build(),
        }
    }
    fn run(&mut self, block: Block) -> BlockExecutionResponse {
        self.inner.run(move |ctx| {
            // Create the app/execution enviroment
            let backend = StateTables {
                table_selector: ctx,
            };
            let app = State::new(backend);

            // Create block response
            let mut response = BlockExecutionResponse {
                block_hash: Default::default(),
                change_epoch: false,
                node_registry_delta: Vec::new(),
                txn_receipts: Vec::with_capacity(block.transactions.len()),
            };

            // Execute each transaction and add the results to the block response
            for txn in &block.transactions {
                let receipt = match app.verify_transaction(txn) {
                    Ok(_) => app.execute_txn(txn.clone()),
                    Err(err) => TransactionResponse::Revert(err),
                };

                // If the transaction moved the epoch forward, aknowledge that in the block response
                if let TransactionResponse::Success(ExecutionData::EpochChange) = receipt {
                    response.change_epoch = true;
                }
                /* Todo(dalton): Check if the transaction resulted in the committee change(Like a current validator getting slashed)
                    if so aknowledge that in the block response
                */
                response.txn_receipts.push(receipt);
            }

            // Return the response
            response
        })
    }

    /// Returns an identical enviroment but with query permissions
    pub fn query_socket(&self) -> Env<QueryPerm> {
        Env {
            inner: self.inner.query(),
        }
    }

    pub fn query_runner(&self) -> QueryRunner {
        QueryRunner::init(self.inner.query())
    }

    /// Seeds the application state with the genesis block
    /// This function will panic if the genesis file cannot be decoded into the correct types
    pub fn genesis(&mut self, config: Config) {
        self.inner.run(|ctx| {
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

            let mut node_table = ctx.get_table::<NodePublicKey, NodeInfo>("node");
            let mut account_table = ctx.get_table::<EthAddress, AccountInfo>("account");
            let mut service_table = ctx.get_table::<ServiceId, Service>("service");
            let mut param_table = ctx.get_table::<ProtocolParams, u128>("parameter");
            let mut committee_table = ctx.get_table::<Epoch, Committee>("committee");
            let mut metadata_table = ctx.get_table::<Metadata, Value>("metadata");
            let mut commodity_prices_table =
                ctx.get_table::<CommodityTypes, HpUfixed<6>>("commodity_prices");
            let mut rep_scores_table = ctx.get_table::<NodePublicKey, u8>("rep_scores");
            let mut total_served_table = ctx.get_table::<Epoch, TotalServed>("total_served");
            let mut current_epoch_served_table =
                ctx.get_table::<NodePublicKey, NodeServed>("current_epoch_served");
            let mut latencies_table =
                ctx.get_table::<(u64, u64), Duration>("latencies");
            let mut pubkey_to_index_table = ctx.get_table::<NodePublicKey, u64>("pubkey_to_index");
            let mut index_to_pubkey_table = ctx.get_table::<u64, NodePublicKey>("index_to_pubkey");

            let protocol_fund_address =
                AccountOwnerPublicKey::from_base64(&genesis.protocol_fund_address).unwrap();
            metadata_table.insert(
                Metadata::ProtocolFundAddress,
                Value::AccountPublicKey(protocol_fund_address.into()),
            );

            let governance_address = AccountOwnerPublicKey::from_base64(&genesis.governance_address)
                .expect("Invalid governance address in genesis, must be a Secp256k1 public key.");
            let governance_address: EthAddress = governance_address.into();
            metadata_table.insert(Metadata::GovernanceAddress,
                Value::AccountPublicKey(governance_address));
            let governance_account = AccountInfo {
                flk_balance: 0u64.into(),
                stables_balance: 0u64.into(),
                bandwidth_balance: 0u64.into(),
                nonce: 0,
            };
            account_table.insert(governance_address,  governance_account);

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
            let mut committee_members = Vec::with_capacity(genesis.committee.len());

            for node in &genesis.committee {
                let mut node_info: NodeInfo = node.into();
                node_info.stake.staked = HpUfixed::<18>::from(genesis.min_stake);
                committee_members.push(node_info.public_key);

                let node_index = match metadata_table.get(Metadata::NextNodeIndex) {
                    Some(Value::NextNodeIndex(index)) => index,
                    _ => 0,
                };
                pubkey_to_index_table.insert(node_info.public_key, node_index);
        index_to_pubkey_table.insert(node_index, node_info.public_key);
                node_table.insert(node_info.public_key, node_info);
                metadata_table.insert(
                    Metadata::NextNodeIndex,
                    Value::NextNodeIndex(node_index + 1),
                );
            }

            committee_table.insert(
                0,
                Committee {
                    ready_to_change: Vec::with_capacity(committee_members.len()),
                    members: committee_members,
                    epoch_end_timestamp: epoch_end,
                },
            );

            for service in &genesis.service {
                let owner_public_key = AccountOwnerPublicKey::from_base64(&service.owner).unwrap();
                service_table.insert(
                    service.id,
                    Service {
                        owner: owner_public_key.into(),
                        commodity_type: service.commodity_type,
                        slashing: (),
                    },
                )
            }

            for account in genesis.account {
                let public_key = AccountOwnerPublicKey::from_base64(&account.public_key).unwrap();
                let info = AccountInfo {
                    flk_balance: account.flk_balance.into(),
                    stables_balance: account.stables_balance.into(),
                    bandwidth_balance: account.bandwidth_balance.into(),
                    nonce: 0,
                };
                account_table.insert(EthAddress::from(public_key), info);
            }

            // add commodity prices
            for commodity_price in genesis.commodity_prices {
                let GenesisPrices { commodity, price } = commodity_price;
                let big_price: HpUfixed<6> = price.into();
                commodity_prices_table.insert(commodity, big_price);
            }

            // add reputation scores
            for (node_public_key_b64, rep_score) in genesis.rep_scores {
                let node_public_key = NodePublicKey::from_base64(&node_public_key_b64)
                    .expect("Failed to parse node public key from genesis.");
                assert!(
                    (0..=100).contains(&rep_score),
                    "Reputation scores must be in range [0, 100]."
                );
                rep_scores_table.insert(node_public_key, rep_score);
            }

            // add node info
            for (node_public_key_b64, node_info) in genesis.node_info {
                let node_public_key = NodePublicKey::from_base64(&node_public_key_b64)
                    .expect("Failed to parse node public key from genesis.");
                node_table.insert(node_public_key, node_info);
            }

            // add total served
            for (epoch, total_served) in genesis.total_served {
                total_served_table.insert(epoch, total_served);
            }

            // add current epoch served
            for (node_public_key_b64, commodity_served) in genesis.current_epoch_served {
                let node_public_key = NodePublicKey::from_base64(&node_public_key_b64)
                    .expect("Failed to parse node public key from genesis.");
                current_epoch_served_table.insert(node_public_key, commodity_served);
            }

            // add latencies
            if let Some(latencies) = genesis.latencies {
                for lat in latencies {
                    let node_public_key_lhs = NodePublicKey::from_base64(&lat.node_public_key_lhs)
                        .expect("Failed to parse node public key from genesis.");
                    let node_public_key_rhs = NodePublicKey::from_base64(&lat.node_public_key_rhs)
                        .expect("Failed to parse node public key from genesis.");
                    assert!(node_public_key_lhs < node_public_key_rhs,
                        "Invalid latency entry, node_public_key_lhs must be smaller than node_public_key_rhs");
                    let index_lhs = pubkey_to_index_table.get(node_public_key_lhs)
                        .expect("Invalid latency entry, node doesn't have an index.");
                    let index_rhs = pubkey_to_index_table.get(node_public_key_rhs)
                        .expect("Invalid latency entry, node doesn't have an index.");
                    latencies_table.insert(
                        (index_lhs, index_rhs),
                        Duration::from_micros(lat.latency_in_microseconds),
                    );
                }
            }

        })
    }
}

impl Default for Env<UpdatePerm> {
    fn default() -> Self {
        Self::new()
    }
}

/// The socket that recieves all update transactions
pub struct UpdateWorker {
    env: Env<UpdatePerm>,
}

impl UpdateWorker {
    pub fn new(env: Env<UpdatePerm>) -> Self {
        Self { env }
    }
}

impl WorkerTrait for UpdateWorker {
    type Request = Block;
    type Response = BlockExecutionResponse;
    fn handle(&mut self, req: Self::Request) -> Self::Response {
        self.env.run(req)
    }
}
