use atomo::{Atomo, AtomoBuilder, DefaultSerdeBackend, QueryPerm, UpdatePerm};
use draco_interfaces::types::{
    Epoch, Metadata, ProtocolParams, QueryRequest, Service, ServiceId, UpdateRequest,
};
use fleek_crypto::{AccountOwnerPublicKey, NodePublicKey};

use crate::{
    state::{AccountInfo, BandwidthInfo, Committee, NodeInfo, State, TransactionResponse},
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
}

impl Env<UpdatePerm> {
    fn run(&mut self, transaction: UpdateRequest) -> TransactionResponse {
        self.inner.run(move |ctx| {
            let backend = StateTables {
                table_selector: ctx,
            };

            let app = State::new(backend);

            app.execute_txn(transaction.clone())
        })
    }

    fn query(&self) -> Env<QueryPerm> {
        Env {
            inner: self.inner.query(),
        }
    }

    // This function will panic if the genesis file cannot be decoded into the correct types
    // fn genesis(&mut self) {
    //     self.atomo.run(|ctx| {
    //         let genesis = Genesis::load().unwrap();

    //         let mut node_table = ctx.get_table::<PublicKey, NodeInfo>("node");
    //         let mut account_table = ctx.get_table::<PublicKey, AccountInfo>("account");
    //         let mut service_table = ctx.get_table::<ServiceId, Service>("service");
    //         let mut param_table = ctx.get_table::<ProtocolParams, u128>("parameter");
    //         let mut committee_table = ctx.get_table::<Epoch, Committee>("committee");

    //         param_table.insert(ProtocolParams::EpochTime, genesis.epoch_time.into());
    //         param_table.insert(ProtocolParams::CommitteeSize, genesis.committee_size.into());
    //         param_table.insert(ProtocolParams::MinimumNodeStake, genesis.min_stake.into());
    //         param_table.insert(
    //             ProtocolParams::EligibilityTime,
    //             genesis.eligibility_time.into(),
    //         );
    //         param_table.insert(ProtocolParams::LockTime, genesis.lock_time.into());
    //         param_table.insert(
    //             ProtocolParams::ProtocolPercentage,
    //             genesis.protocol_percentage.into(),
    //         );
    //         param_table.insert(ProtocolParams::MaxInflation, genesis.max_inflation.into());
    //         param_table.insert(ProtocolParams::MinInflation, genesis.min_inflation.into());
    //         param_table.insert(
    //             ProtocolParams::ConsumerRebate,
    //             genesis.consumer_rebate.into(),
    //         );

    //         let epoch_end = genesis.epoch_time + genesis.epoch_start;
    //         let mut committee_members = Vec::with_capacity(genesis.committee.len());

    //         for node in &genesis.committee {
    //             let stake = node.staking;
    //             let node_info: NodeInfo = node.into();

    //             let owner = node_info.owner.clone();
    //             committee_members.push(owner.clone());

    //             // If stake amount is specified add it to the owner account
    //             if let Some(stake) = stake {
    //                 account_table.insert(
    //                     owner.clone(),
    //                     AccountInfo {
    //                         flk_balance: 0,
    //                         bandwidth_balance: 0,
    //                         nonce: 0,
    //                         staking: Staking {
    //                             staked: stake.into(),
    //                             locked: 0,
    //                             locked_until: 0,
    //                         },
    //                     },
    //                 )
    //             }
    //             node_table.insert(owner, node_info);
    //         }

    //         committee_table.insert(
    //             0,
    //             Committee {
    //                 ready_to_change: Vec::with_capacity(committee_members.len()),
    //                 members: committee_members,
    //                 epoch_end_timestamp: epoch_end.try_into().unwrap(),
    //             },
    //         );

    //         for service in &genesis.service {
    //             service_table.insert(
    //                 service.id,
    //                 Service {
    //                     commodity_price: service.commodity_price.into(),
    //                     slashing: (),
    //                 },
    //             )
    //         }

    //         for account in genesis.account {
    //             let public_key = PublicKey::decode_base64(&account.public_key).unwrap();
    //             let info = AccountInfo {
    //                 flk_balance: account.flk_balance.into(),
    //                 bandwidth_balance: account.bandwidth_balance.into(),
    //                 nonce: 0,
    //                 staking: Staking {
    //                     staked: account.staked.into(),
    //                     locked: 0,
    //                     locked_until: 0,
    //                 },
    //             };
    //             account_table.insert(public_key, info);
    //         }
    //     })
    // }
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
