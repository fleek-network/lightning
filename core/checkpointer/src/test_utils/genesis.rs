use std::collections::HashMap;
use std::str::FromStr;

use fleek_crypto::{AccountOwnerSecretKey, EthAddress, SecretKey};
use hp_fixed::unsigned::HpUfixed;
use lightning_interfaces::types::{
    ChainId,
    CommodityTypes,
    Genesis,
    GenesisAccount,
    GenesisNode,
    GenesisPrices,
    GenesisService,
    NodePorts,
    Staking,
};
use lightning_interfaces::KeystoreInterface;
use ready::ReadyWaiter;

use super::TestNode;

#[derive(Clone)]
pub struct TestGenesisBuilder {
    chain_id: ChainId,
    owner_secret_key: AccountOwnerSecretKey,
    protocol_address: EthAddress,
    nodes: Vec<GenesisNode>,
}

impl Default for TestGenesisBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl TestGenesisBuilder {
    pub fn new() -> Self {
        let protocol_secret_key = AccountOwnerSecretKey::generate();
        Self {
            chain_id: 1337,
            owner_secret_key: AccountOwnerSecretKey::generate(),
            nodes: Vec::new(),
            protocol_address: protocol_secret_key.to_pk().into(),
        }
    }

    pub fn with_chain_id(self, chain_id: ChainId) -> Self {
        Self { chain_id, ..self }
    }

    pub fn with_owner(self, secret_key: AccountOwnerSecretKey) -> Self {
        Self {
            owner_secret_key: secret_key,
            ..self
        }
    }

    pub fn with_protocol_address(self, address: EthAddress) -> Self {
        Self {
            protocol_address: address,
            ..self
        }
    }

    pub fn with_node(mut self, node: &TestNode) -> Self {
        let node_secret_key = node.keystore.get_ed25519_sk();
        let node_public_key = node_secret_key.to_pk();
        let consensus_secret_key = node.keystore.get_bls_sk();
        let consensus_public_key = consensus_secret_key.to_pk();
        let node_domain = "127.0.0.1".parse().unwrap();
        let ready = node.before_genesis_ready.state().expect("node not ready");
        let ports = NodePorts {
            pool: ready.pool_listen_address.port(),
            ..Default::default()
        };

        self.nodes.push(GenesisNode::new(
            self.owner_secret_key.to_pk().into(),
            node_public_key,
            node_domain,
            consensus_public_key,
            node_domain,
            node_public_key,
            ports,
            Some(Staking {
                staked: HpUfixed::<18>::from(1000u32),
                stake_locked_until: 0,
                locked: HpUfixed::<18>::zero(),
                locked_until: 0,
            }),
            true,
        ));

        self
    }

    pub fn build(self) -> Genesis {
        Genesis {
            chain_id: self.chain_id,
            epoch_start: 1684276288383,
            epoch_time: 120000,
            committee_size: 10,
            node_count: 10,
            min_stake: 1000,
            eligibility_time: 1,
            lock_time: 5,
            protocol_share: 0,
            node_share: 80,
            service_builder_share: 20,
            max_inflation: 10,
            consumer_rebate: 0,
            max_boost: 4,
            max_lock_time: 1460,
            supply_at_genesis: 1000000,
            min_num_measurements: 2,
            protocol_fund_address: self.protocol_address,
            governance_address: self.protocol_address,
            node_info: self.nodes,
            service: vec![
                GenesisService {
                    id: 0,
                    owner: EthAddress::from_str("0xDC0A31F9eeb151f82BF1eE6831095284fC215Ee7")
                        .unwrap(),
                    commodity_type: CommodityTypes::Bandwidth,
                },
                GenesisService {
                    id: 1,
                    owner: EthAddress::from_str("0x684166BDbf530a256d7c92Fa0a4128669aFd9B9F")
                        .unwrap(),
                    commodity_type: CommodityTypes::Compute,
                },
            ],
            account: vec![GenesisAccount {
                public_key: self.owner_secret_key.to_pk().into(),
                flk_balance: HpUfixed::<18>::from(100690000000000000000u128),
                stables_balance: 100,
                bandwidth_balance: 100,
            }],
            client: HashMap::new(),
            commodity_prices: vec![
                GenesisPrices {
                    commodity: CommodityTypes::Bandwidth,
                    price: 0.1,
                },
                GenesisPrices {
                    commodity: CommodityTypes::Compute,
                    price: 0.2,
                },
            ],
            total_served: HashMap::new(),
            latencies: None,
        }
    }
}
