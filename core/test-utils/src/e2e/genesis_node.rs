use fleek_crypto::{ConsensusSecretKey, EthAddress, NodeSecretKey, SecretKey};
use hp_fixed::unsigned::HpUfixed;
use lightning_interfaces::types::{GenesisNode, NodePorts, Staking};

pub struct TestGenesisNodeBuilder {
    owner: Option<EthAddress>,
    node_secret_key: Option<NodeSecretKey>,
    consensus_secret_key: Option<ConsensusSecretKey>,
    stake: Option<Staking>,
    is_committee: Option<bool>,
    node_ports: Option<NodePorts>,
}

impl TestGenesisNodeBuilder {
    pub fn new() -> Self {
        Self {
            stake: None,
            owner: None,
            node_secret_key: None,
            consensus_secret_key: None,
            is_committee: None,
            node_ports: None,
        }
    }

    pub fn with_owner(mut self, owner: EthAddress) -> Self {
        self.owner = Some(owner);
        self
    }

    pub fn with_node_secret_key(mut self, sk: NodeSecretKey) -> Self {
        self.node_secret_key = Some(sk);
        self
    }

    pub fn with_consensus_secret_key(mut self, sk: ConsensusSecretKey) -> Self {
        self.consensus_secret_key = Some(sk);
        self
    }

    pub fn with_stake(mut self, stake: Staking) -> Self {
        self.stake = Some(stake);
        self
    }

    pub fn with_is_committee(mut self, is_committee: bool) -> Self {
        self.is_committee = Some(is_committee);
        self
    }

    pub fn with_node_ports(mut self, node_ports: NodePorts) -> Self {
        self.node_ports = Some(node_ports);
        self
    }

    pub fn build(self) -> GenesisNode {
        let node_secret_key = self.node_secret_key.unwrap_or_else(NodeSecretKey::generate);
        let consensus_secret_key = self
            .consensus_secret_key
            .unwrap_or_else(ConsensusSecretKey::generate);
        GenesisNode {
            owner: self.owner.unwrap_or_default(),
            primary_public_key: node_secret_key.to_pk(),
            consensus_public_key: consensus_secret_key.to_pk(),
            primary_domain: "127.0.0.1".parse().unwrap(),
            worker_domain: "127.0.0.1".parse().unwrap(),
            worker_public_key: node_secret_key.to_pk(),
            ports: self.node_ports.unwrap_or_default(),
            stake: self.stake.unwrap_or_else(|| Staking {
                staked: HpUfixed::<18>::from(1000u32),
                stake_locked_until: 0,
                locked: HpUfixed::<18>::zero(),
                locked_until: 0,
            }),
            reputation: None,
            current_epoch_served: None,
            genesis_committee: self.is_committee.unwrap_or(true),
        }
    }
}

impl Default for TestGenesisNodeBuilder {
    fn default() -> Self {
        Self::new()
    }
}
