use std::collections::HashMap;

use anyhow::{Context, Result};
use fleek_crypto::{AccountOwnerPublicKey, ConsensusPublicKey, NodePublicKey, PublicKey};
use lightning_interfaces::types::{
    CommodityTypes,
    Epoch,
    NodeInfo,
    NodeServed,
    Staking,
    TotalServed,
    Worker,
};
use multiaddr::Multiaddr;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Genesis {
    pub epoch_start: u64,
    pub epoch_time: u64,
    pub committee_size: u64,
    pub min_stake: u64,
    pub eligibility_time: u64,
    pub lock_time: Epoch,
    pub max_inflation: u16,
    pub protocol_share: u16,
    pub node_share: u16,
    pub service_builder_share: u16,
    pub consumer_rebate: u64,
    pub max_boost: u16,
    pub max_lock_time: u64,
    pub committee: Vec<GenesisCommittee>,
    pub service: Vec<GenesisService>,
    pub account: Vec<GenesisAccount>,
    pub commodity_prices: Vec<GenesisPrices>,
    pub supply_at_genesis: u64,
    pub protocol_fund_address: String,
    pub governance_address: String,
    pub rep_scores: HashMap<String, u8>,
    pub node_info: HashMap<String, NodeInfo>,
    pub total_served: HashMap<Epoch, TotalServed>,
    pub current_epoch_served: HashMap<String, NodeServed>,
    pub latencies: Option<Vec<GenesisLatency>>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GenesisAccount {
    pub public_key: String,
    pub flk_balance: u64,
    pub stables_balance: u64,
    pub bandwidth_balance: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GenesisService {
    pub id: u32,
    pub owner: String,
    pub commodity_type: CommodityTypes,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GenesisCommittee {
    owner: String,
    pub primary_public_key: String,
    primary_address: String,
    consensus_public_key: String,
    worker_address: String,
    worker_public_key: String,
    worker_mempool: String,
    pub staking: Option<u64>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GenesisPrices {
    pub commodity: CommodityTypes,
    pub price: f64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GenesisLatency {
    pub node_public_key_lhs: String,
    pub node_public_key_rhs: String,
    pub latency_in_microseconds: u64,
}

impl Genesis {
    /// Load the genesis file.
    pub fn load() -> Result<Genesis> {
        let raw = include_str!("../genesis.toml");
        toml::from_str(raw).context("Failed to parse genesis file")
    }
}

#[test]
fn test() {
    Genesis::load().unwrap();
}

impl From<&GenesisCommittee> for NodeInfo {
    fn from(value: &GenesisCommittee) -> Self {
        let owner = AccountOwnerPublicKey::from_base64(&value.owner).unwrap();
        let public_key = NodePublicKey::from_base64(&value.primary_public_key).unwrap();
        let consensus_key = ConsensusPublicKey::from_base64(&value.consensus_public_key).unwrap();

        let domain: Multiaddr = value.primary_address.parse().unwrap();

        let worker = Worker {
            public_key: NodePublicKey::from_base64(&value.worker_public_key).unwrap(),
            address: value.worker_address.parse().unwrap(),
            mempool: value.worker_mempool.parse().unwrap(),
        };

        NodeInfo {
            owner: owner.into(),
            public_key,
            consensus_key,
            domain,
            workers: [worker].to_vec(),
            staked_since: 0,
            stake: Staking::default(),
            nonce: 0,
        }
    }
}

impl From<NodeInfo> for GenesisCommittee {
    fn from(value: NodeInfo) -> Self {
        let worker = value.workers.first().expect("At least one worker.");

        GenesisCommittee {
            owner: value.owner.to_string(),
            primary_public_key: value.public_key.to_base64(),
            primary_address: value.domain.to_string(),
            consensus_public_key: value.consensus_key.to_base64(),
            worker_address: worker.address.to_string(),
            worker_public_key: worker.public_key.to_base64(),
            worker_mempool: worker.mempool.to_string(),
            staking: Some(value.stake.staked.try_into().unwrap()),
        }
    }
}

impl GenesisCommittee {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        owner: String,
        primary_public_key: String,
        primary_address: String,
        consensus_public_key: String,
        worker_address: String,
        worker_public_key: String,
        worker_mempool: String,
        staking: Option<u64>,
    ) -> Self {
        Self {
            owner,
            primary_public_key,
            primary_address,
            consensus_public_key,
            worker_address,
            worker_public_key,
            worker_mempool,
            staking,
        }
    }
}
