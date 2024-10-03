use std::collections::HashMap;
use std::fs;
use std::net::IpAddr;
use std::time::Duration;

use anyhow::{Context, Result};
use fleek_crypto::{ClientPublicKey, ConsensusPublicKey, EthAddress, NodePublicKey};
use hp_fixed::unsigned::HpUfixed;
use resolved_pathbuf::ResolvedPathBuf;
use serde::{self, Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};

use crate::{
    CommodityServed,
    CommodityTypes,
    Epoch,
    NodeInfo,
    NodePorts,
    NodeServed,
    Participation,
    Staking,
    TotalServed,
};

#[serde_as]
#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
pub struct Genesis {
    pub chain_id: u32,
    pub epoch_start: u64,
    pub epoch_time: u64,
    pub epochs_per_year: u64,
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
    pub min_num_measurements: u64,
    pub node_info: Vec<GenesisNode>,
    pub service: Vec<GenesisService>,
    pub account: Vec<GenesisAccount>,
    // We need to customize this because the TOML crate requires string keys for maps.
    #[serde_as(as = "HashMap<DisplayFromStr, _>")]
    pub client: HashMap<ClientPublicKey, EthAddress>,
    pub commodity_prices: Vec<GenesisPrices>,
    pub supply_at_genesis: u64,
    pub protocol_fund_address: EthAddress,
    pub governance_address: EthAddress,
    // We need to customize this because the TOML crate requires string keys for maps.
    #[serde_as(as = "HashMap<DisplayFromStr, _>")]
    pub total_served: HashMap<Epoch, GenesisTotalServed>,
    pub latencies: Option<Vec<GenesisLatency>>,
    #[serde(with = "humantime_serde")]
    pub reputation_ping_timeout: Duration,
}

impl Genesis {
    pub fn load_from_file(path: ResolvedPathBuf) -> Result<Self> {
        let raw = fs::read_to_string(path)?;
        let genesis = toml::from_str(&raw).context("Failed to parse genesis file")?;
        Ok(genesis)
    }

    pub fn write_to_file(&self, path: ResolvedPathBuf) -> Result<()> {
        let raw = toml::to_string_pretty(self)?;
        fs::write(path, raw)?;
        Ok(())
    }

    pub fn write_to_dir(&self, dir: ResolvedPathBuf) -> Result<ResolvedPathBuf> {
        let path: ResolvedPathBuf = dir.join("genesis.toml").try_into()?;
        self.write_to_file(path.clone())?;
        Ok(path)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct GenesisAccount {
    pub public_key: EthAddress,
    pub flk_balance: HpUfixed<18>,
    pub stables_balance: u64,
    pub bandwidth_balance: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct GenesisService {
    pub id: u32,
    pub owner: EthAddress,
    pub commodity_type: CommodityTypes,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
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
    pub current_epoch_served: Option<GenesisNodeServed>,
    pub genesis_committee: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct GenesisPrices {
    pub commodity: CommodityTypes,
    pub price: f64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct GenesisLatency {
    pub node_public_key_lhs: NodePublicKey,
    pub node_public_key_rhs: NodePublicKey,
    pub latency_in_millis: u64,
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

#[serde_as]
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone, Default)]
pub struct GenesisTotalServed {
    // We need to customize this because u128 is not supported by the TOML crate:
    // https://github.com/toml-rs/toml-rs/issues/212
    #[serde_as(as = "Vec<DisplayFromStr>")]
    pub served: CommodityServed,
    pub reward_pool: HpUfixed<6>,
}

impl From<GenesisTotalServed> for TotalServed {
    fn from(genesis_total_served: GenesisTotalServed) -> Self {
        Self {
            served: genesis_total_served.served,
            reward_pool: genesis_total_served.reward_pool,
        }
    }
}

impl From<TotalServed> for GenesisTotalServed {
    fn from(total_served: TotalServed) -> Self {
        Self {
            served: total_served.served,
            reward_pool: total_served.reward_pool,
        }
    }
}

#[serde_as]
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone, Default)]
pub struct GenesisNodeServed {
    // We need to customize this because u128 is not supported by the TOML crate:
    // https://github.com/toml-rs/toml-rs/issues/212
    #[serde_as(as = "Vec<DisplayFromStr>")]
    pub served: CommodityServed,
    pub stables_revenue: HpUfixed<6>,
}

impl From<GenesisNodeServed> for NodeServed {
    fn from(genesis_node_served: GenesisNodeServed) -> Self {
        Self {
            served: genesis_node_served.served,
            stables_revenue: genesis_node_served.stables_revenue,
        }
    }
}

#[cfg(test)]
mod genesis_tests {
    use resolved_pathbuf::ResolvedPathBuf;
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn write_to_file_load_from_file() {
        let temp_dir = tempdir().unwrap();
        let genesis = Genesis {
            chain_id: 1337,
            ..Genesis::default()
        };
        let genesis_path: ResolvedPathBuf =
            temp_dir.path().join("genesis.toml").try_into().unwrap();
        genesis.write_to_file(genesis_path.clone()).unwrap();
        let loaded_genesis = Genesis::load_from_file(genesis_path).unwrap();
        assert_eq!(genesis, loaded_genesis);
    }

    #[test]
    fn write_to_dir_load_from_file() {
        let temp_dir = tempdir().unwrap();
        let genesis = Genesis {
            chain_id: 1337,
            ..Genesis::default()
        };
        let genesis_path: ResolvedPathBuf = genesis
            .write_to_dir(temp_dir.path().to_path_buf().try_into().unwrap())
            .unwrap();
        let loaded_genesis = Genesis::load_from_file(genesis_path).unwrap();
        assert_eq!(genesis, loaded_genesis);
    }
}
