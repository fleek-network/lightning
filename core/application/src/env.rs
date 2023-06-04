use affair::{AsyncWorker, Worker as WorkerTrait};
use atomo::{Atomo, AtomoBuilder, DefaultSerdeBackend, QueryPerm, UpdatePerm};
use draco_interfaces::{
    types::{
        AccountInfo, Block, Epoch, ExecutionData, Metadata, NodeInfo, ProtocolParams, QueryRequest,
        QueryResponse, Service, ServiceId, TransactionResponse,
    },
    BlockExecutionResponse,
};
use fastcrypto::{ed25519::Ed25519PublicKey, traits::EncodeDecodeBase64};
use fleek_crypto::{AccountOwnerPublicKey, NodePublicKey};

use crate::{
    genesis::Genesis,
    state::{BandwidthInfo, Committee, State},
    table::StateTables,
};

pub struct Env<P> {
    inner: Atomo<P>,
}

impl Env<UpdatePerm> {
    pub fn new() -> Self {
        let atomo = AtomoBuilder::<DefaultSerdeBackend>::new()
            .with_table::<Metadata, u64>("metadata")
            .with_table::<AccountOwnerPublicKey, AccountInfo>("account")
            .with_table::<NodePublicKey, NodeInfo>("node")
            .with_table::<Epoch, Committee>("committee")
            .with_table::<Epoch, BandwidthInfo>("bandwidth")
            .with_table::<ServiceId, Service>("service")
            .with_table::<ProtocolParams, u128>("parameter")
            .build();

        Self { inner: atomo }
    }
    fn run(&mut self, block: Block) -> BlockExecutionResponse {
        self.inner.run(move |ctx| {
            let backend = StateTables {
                table_selector: ctx,
            };

            let mut response = BlockExecutionResponse {
                block_hash: Default::default(),
                change_epoch: false,
                node_registry_delta: Vec::new(),
                txn_receipts: Vec::with_capacity(block.transactions.len()),
            };

            let app = State::new(backend);

            for txn in &block.transactions {
                let receipt = app.execute_txn(txn.clone());
                if let TransactionResponse::Success(ExecutionData::EpochChange) = receipt {
                    response.change_epoch = true;
                }
                response.txn_receipts.push(receipt);
            }

            response
        })
    }

    pub fn query(&self) -> Env<QueryPerm> {
        Env {
            inner: self.inner.query(),
        }
    }

    /// This function will panic if the genesis file cannot be decoded into the correct types
    pub fn genesis(&mut self) {
        self.inner.run(|ctx| {
            let genesis = Genesis::load().unwrap();

            let mut node_table = ctx.get_table::<NodePublicKey, NodeInfo>("node");
            let mut account_table = ctx.get_table::<AccountOwnerPublicKey, AccountInfo>("account");
            let mut service_table = ctx.get_table::<ServiceId, Service>("service");
            let mut param_table = ctx.get_table::<ProtocolParams, u128>("parameter");
            let mut committee_table = ctx.get_table::<Epoch, Committee>("committee");

            param_table.insert(ProtocolParams::EpochTime, genesis.epoch_time as u128);
            param_table.insert(
                ProtocolParams::CommitteeSize,
                genesis.committee_size as u128,
            );
            param_table.insert(ProtocolParams::MinimumNodeStake, genesis.min_stake as u128);
            param_table.insert(
                ProtocolParams::EligibilityTime,
                genesis.eligibility_time as u128,
            );
            param_table.insert(ProtocolParams::LockTime, genesis.lock_time as u128);
            param_table.insert(
                ProtocolParams::ProtocolPercentage,
                genesis.protocol_percentage as u128,
            );
            param_table.insert(ProtocolParams::MaxInflation, genesis.max_inflation as u128);
            param_table.insert(ProtocolParams::MinInflation, genesis.min_inflation as u128);
            param_table.insert(
                ProtocolParams::ConsumerRebate,
                genesis.consumer_rebate as u128,
            );

            let epoch_end = genesis.epoch_time + genesis.epoch_start;
            let mut committee_members = Vec::with_capacity(genesis.committee.len());

            for node in &genesis.committee {
                let mut node_info: NodeInfo = node.into();
                node_info.stake.staked = genesis.min_stake as u128;
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
                        commodity_price: service.commodity_price.into(),
                        slashing: (),
                    },
                )
            }

            for account in genesis.account {
                let public_key: AccountOwnerPublicKey =
                    Ed25519PublicKey::decode_base64(&account.public_key)
                        .unwrap()
                        .0
                        .to_bytes()
                        .into();
                let info = AccountInfo {
                    flk_balance: account.flk_balance.into(),
                    bandwidth_balance: account.bandwidth_balance.into(),
                    nonce: 0,
                };
                account_table.insert(public_key, info);
            }
        })
    }
}

impl Default for Env<UpdatePerm> {
    fn default() -> Self {
        Self::new()
    }
}

impl Env<QueryPerm> {
    fn run(&self, transaction: QueryRequest) -> TransactionResponse {
        self.inner.run(|ctx| {
            let backend = StateTables {
                table_selector: ctx,
            };
            let app = State::new(backend);
            app.execute_query(transaction.clone())
        })
    }
}

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
        // 1. Verify Signature and Nonce
        // Note(Dalton), this check will probably be moved to execute_txn function, and backend Arc
        // will probably only be stored there as well if let Err(err) =
        // self.backend.verify_transaction(&req) {     return
        // TransactionResponse::Revert(err); }
        // 2. Execute the transaction based on the transaction type
        self.env.run(req)
    }
}

pub struct QueryWorker {
    env: Env<QueryPerm>,
}

impl QueryWorker {
    pub fn new(env: Env<QueryPerm>) -> Self {
        Self { env }
    }
}

#[async_trait::async_trait]
impl AsyncWorker for QueryWorker {
    type Request = QueryRequest;
    type Response = QueryResponse;
    async fn handle(&mut self, req: Self::Request) -> Self::Response {
        // 1. Just execute transaction and return results, no need to do signature verification
        let response = self.env.run(req);
        bincode::serialize(&response).unwrap()
    }
}
