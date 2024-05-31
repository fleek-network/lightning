use std::collections::HashMap;
use std::net::IpAddr;

use anyhow::{Context, Result};
use fleek_crypto::{ClientPublicKey, ConsensusPublicKey, EthAddress, NodePublicKey};
use hp_fixed::unsigned::HpUfixed;
use lightning_interfaces::types::{
    CommodityTypes,
    Epoch,
    NodeInfo,
    NodePorts,
    NodeServed,
    Participation,
    Staking,
    TotalServed,
};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Genesis {
    pub chain_id: u32,
    pub epoch_start: u64,
    pub epoch_time: u64,
    pub committee_size: u64,
    pub node_count: u64,
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
    pub node_info: Vec<GenesisNode>,
    pub service: Vec<GenesisService>,
    pub account: Vec<GenesisAccount>,
    pub client: HashMap<ClientPublicKey, EthAddress>,
    pub commodity_prices: Vec<GenesisPrices>,
    pub supply_at_genesis: u64,
    pub protocol_fund_address: EthAddress,
    pub governance_address: EthAddress,
    pub total_served: HashMap<Epoch, TotalServed>,
    pub latencies: Option<Vec<GenesisLatency>>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GenesisAccount {
    pub public_key: EthAddress,
    pub flk_balance: HpUfixed<18>,
    pub stables_balance: u64,
    pub bandwidth_balance: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GenesisService {
    pub id: u32,
    pub owner: EthAddress,
    pub commodity_type: CommodityTypes,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GenesisNode {
    pub owner: EthAddress,
    pub primary_public_key: NodePublicKey,
    pub consensus_public_key: ConsensusPublicKey,
    pub primary_domain: IpAddr,
    pub worker_domain: IpAddr,
    pub worker_public_key: NodePublicKey,
    pub ports: NodePorts,
    pub stake: Staking,
    pub reputation: Option<u8>,
    pub current_epoch_served: Option<NodeServed>,
    pub genesis_committee: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GenesisPrices {
    pub commodity: CommodityTypes,
    pub price: f64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GenesisLatency {
    pub node_public_key_lhs: NodePublicKey,
    pub node_public_key_rhs: NodePublicKey,
    pub latency_in_millis: u64,
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

impl From<&GenesisNode> for NodeInfo {
    fn from(value: &GenesisNode) -> Self {
        NodeInfo {
            owner: value.owner,
            public_key: value.primary_public_key,
            consensus_key: value.consensus_public_key,
            domain: value.primary_domain,
            worker_domain: value.worker_domain,
            worker_public_key: value.worker_public_key,
            staked_since: 0,
            stake: value.stake.clone(),
            participation: Participation::True,
            nonce: 0,
            ports: value.ports.clone(),
        }
    }
}

impl From<NodeInfo> for GenesisNode {
    fn from(value: NodeInfo) -> Self {
        GenesisNode {
            owner: value.owner,
            primary_public_key: value.public_key,
            primary_domain: value.domain,
            consensus_public_key: value.consensus_key,
            worker_domain: value.worker_domain,
            worker_public_key: value.worker_public_key,
            stake: value.stake,
            ports: value.ports,
            reputation: None,
            current_epoch_served: None,
            genesis_committee: false,
        }
    }
}

impl GenesisNode {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        owner: EthAddress,
        primary_public_key: NodePublicKey,
        primary_domain: IpAddr,
        consensus_public_key: ConsensusPublicKey,
        worker_domain: IpAddr,
        worker_public_key: NodePublicKey,
        ports: NodePorts,
        stake: Option<Staking>,
        is_committee: bool,
    ) -> Self {
        Self {
            owner,
            primary_public_key,
            primary_domain,
            consensus_public_key,
            worker_domain,
            worker_public_key,
            ports,
            stake: stake.unwrap_or_default(),
            reputation: None,
            current_epoch_served: None,
            genesis_committee: is_committee,
        }
    }
}
