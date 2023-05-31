use anyhow::{Context, Result};
use fastcrypto::{
    bls12381::min_sig::BLS12381PublicKey, ed25519::Ed25519PublicKey, traits::EncodeDecodeBase64,
};
use multiaddr::Multiaddr;
use serde::{Deserialize, Serialize};

use crate::state::{NodeInfo, Worker};

#[derive(Serialize, Deserialize, Debug)]
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

#[derive(Serialize, Deserialize, Debug)]
pub struct GenesisAccount {
    pub public_key: String,
    pub flk_balance: u64,
    pub bandwidth_balance: u64,
    pub staked: u64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct GenesisService {
    pub id: u64,
    pub commodity_price: u64,
}

#[derive(Serialize, Deserialize, Debug)]
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
    let gen = Genesis::load().unwrap();
    println!("{:?}", gen);
}

impl From<&GenesisCommittee> for NodeInfo {
    fn from(value: &GenesisCommittee) -> Self {
        let owner = Ed25519PublicKey::decode_base64(&value.owner)
            .unwrap()
            .0
            .to_bytes();

        let public_key = BLS12381PublicKey::decode_base64(&value.primary_public_key)
            .unwrap()
            .bytes
            .into_inner()
            .unwrap();

        let network_key = Ed25519PublicKey::decode_base64(&value.network_key)
            .unwrap()
            .0
            .to_bytes();

        let domain: Multiaddr = value.primary_address.parse().unwrap();

        let worker = Worker {
            public_key: Ed25519PublicKey::decode_base64(&value.worker_public_key)
                .unwrap()
                .0
                .to_bytes(),
            address: value.worker_address.parse().unwrap(),
            mempool: value.worker_mempool.parse().unwrap(),
        };

        NodeInfo {
            owner,
            public_key,
            network_key,
            domain,
            workers: [worker].to_vec(),
        }
    }
}
