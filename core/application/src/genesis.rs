use anyhow::{Context, Result};
use draco_interfaces::types::{CommodityTypes, NodeInfo, Staking, Worker};
use fastcrypto::{
    bls12381::min_sig::BLS12381PublicKey, ed25519::Ed25519PublicKey, traits::EncodeDecodeBase64,
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
    pub lock_time: u64,
    pub protocol_percentage: u64,
    pub max_inflation: u64,
    pub min_inflation: u64,
    pub consumer_rebate: u64,
    pub committee: Vec<GenesisCommittee>,
    pub service: Vec<GenesisService>,
    pub account: Vec<GenesisAccount>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GenesisAccount {
    pub public_key: String,
    pub flk_balance: u64,
    pub bandwidth_balance: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GenesisService {
    pub id: u32,
    pub commodity_type: CommodityTypes,
    pub commodity_price: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GenesisCommittee {
    owner: String,
    primary_public_key: String,
    primary_address: String,
    network_key: String,
    worker_address: String,
    worker_public_key: String,
    worker_mempool: String,
    pub staking: Option<u64>,
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
        let owner = Ed25519PublicKey::decode_base64(&value.owner)
            .unwrap()
            .0
            .to_bytes();

        let public_key = BLS12381PublicKey::decode_base64(&value.primary_public_key)
            .unwrap()
            .pubkey
            .to_bytes();

        let network_key = Ed25519PublicKey::decode_base64(&value.network_key)
            .unwrap()
            .0
            .to_bytes();

        let domain: Multiaddr = value.primary_address.parse().unwrap();

        let worker = Worker {
            public_key: Ed25519PublicKey::decode_base64(&value.worker_public_key)
                .unwrap()
                .0
                .to_bytes()
                .into(),
            address: value.worker_address.parse().unwrap(),
            mempool: value.worker_mempool.parse().unwrap(),
        };

        NodeInfo {
            owner: owner.into(),
            public_key: public_key.into(),
            network_key: network_key.into(),
            domain,
            workers: [worker].to_vec(),
            staked_since: 0,
            stake: Staking::default(),
            nonce: 0,
        }
    }
}
