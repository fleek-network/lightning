use std::time::SystemTime;

use affair::Worker as WorkerTrait;
use atomo::{Atomo, AtomoBuilder, DefaultSerdeBackend, QueryPerm, UpdatePerm};
use draco_interfaces::{
    types::{
        AccountInfo, Block, CommodityServed, CommodityTypes, Epoch, ExecutionData, Metadata,
        NodeInfo, ProtocolParams, ReportedReputationMeasurements, Service, ServiceId, TotalServed,
        TransactionResponse, Value,
    },
    BlockExecutionResponse,
};
use fastcrypto::{secp256k1::Secp256k1PublicKey, traits::EncodeDecodeBase64};
use fleek_crypto::{AccountOwnerPublicKey, ClientPublicKey, EthAddress, NodePublicKey, PublicKey};
use hp_float::unsigned::HpUfloat;

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
        let atomo = AtomoBuilder::<DefaultSerdeBackend>::new()
            .with_table::<Metadata, Value>("metadata")
            .with_table::<EthAddress, AccountInfo>("account")
            .with_table::<ClientPublicKey, EthAddress>("client_keys")
            .with_table::<NodePublicKey, NodeInfo>("node")
            .with_table::<Epoch, Committee>("committee")
            .with_table::<ServiceId, Service>("service")
            .with_table::<ProtocolParams, u128>("parameter")
            .with_table::<NodePublicKey, Vec<ReportedReputationMeasurements>>("rep_measurements")
            .with_table::<NodePublicKey, u8>("rep_scores")
            .with_table::<NodePublicKey, CommodityServed>("current_epoch_served")
            .with_table::<NodePublicKey, CommodityServed>("last_epoch_served")
            .with_table::<Epoch, TotalServed>("total_served")
            .with_table::<CommodityTypes, HpUfloat<6>>("commodity_prices")
            .enable_iter("current_epoch_served")
            .enable_iter("rep_measurements")
            .enable_iter("rep_scores")
            .build();

        Self { inner: atomo }
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
                ctx.get_table::<CommodityTypes, HpUfloat<6>>("commodity_prices");
            let mut rep_scores_table = ctx.get_table::<NodePublicKey, u8>("rep_scores");

            let protocol_fund_address: AccountOwnerPublicKey =
                Secp256k1PublicKey::decode_base64(&genesis.protocol_fund_address)
                    .unwrap()
                    .into();
            metadata_table.insert(
                Metadata::ProtocolFundAddress,
                Value::AccountPublicKey(protocol_fund_address.into()),
            );

            let supply_at_genesis: HpUfloat<18> = HpUfloat::from(genesis.supply_at_genesis);
            metadata_table.insert(
                Metadata::TotalSupply,
                Value::HpUfloat(supply_at_genesis.clone()),
            );
            metadata_table.insert(
                Metadata::SupplyYearStart,
                Value::HpUfloat(supply_at_genesis),
            );
            param_table.insert(ProtocolParams::MaxBoost, genesis.max_boost as u128);
            param_table.insert(ProtocolParams::MaxLockTime, genesis.max_lock_time as u128);
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
                ProtocolParams::ValidatorShare,
                genesis.validator_share as u128,
            );
            param_table.insert(
                ProtocolParams::EligibilityTime,
                genesis.eligibility_time as u128,
            );
            param_table.insert(
                ProtocolParams::ConsumerRebate,
                genesis.consumer_rebate as u128,
            );
            param_table.insert(
                ProtocolParams::CommitteeSize,
                genesis.committee_size as u128,
            );

            let epoch_end: u64 = genesis.epoch_time + genesis.epoch_start;
            let mut committee_members = Vec::with_capacity(genesis.committee.len());

            for node in &genesis.committee {
                let mut node_info: NodeInfo = node.into();
                node_info.stake.staked = HpUfloat::<18>::from(genesis.min_stake);
                committee_members.push(node_info.public_key);

                node_table.insert(node_info.public_key, node_info);
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
                service_table.insert(
                    service.id,
                    Service {
                        commodity_type: service.commodity_type,
                        slashing: (),
                    },
                )
            }

            for account in genesis.account {
                let public_key: AccountOwnerPublicKey =
                    Secp256k1PublicKey::decode_base64(&account.public_key)
                        .unwrap()
                        .into();
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
                let big_price: HpUfloat<6> = price.into();
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
