use std::collections::HashMap;
use std::fs;
use std::net::SocketAddr;
use std::path::Path;
use std::str::FromStr;

use fleek_crypto::{
    AccountOwnerSecretKey,
    ConsensusPublicKey,
    ConsensusSecretKey,
    EthAddress,
    NodePublicKey,
    NodeSecretKey,
    PublicKey,
    SecretKey,
};
use futures::future::try_join_all;
use hp_fixed::unsigned::HpUfixed;
use lightning_application::app::Application;
use lightning_application::config::{Config as AppConfig, Mode};
use lightning_application::genesis::Genesis;
use lightning_consensus::config::Config as ConsensusConfig;
use lightning_consensus::consensus::Consensus;
use lightning_dht::config::{Bootstrapper, Config as DhtConfig};
use lightning_dht::dht::Dht;
use lightning_handshake::server::{HandshakeServerConfig, TcpHandshakeServer};
use lightning_interfaces::types::{NodeInfo, Staking, Worker};
use lightning_interfaces::ConfigProviderInterface;
use lightning_node::config::TomlConfigProvider;
use lightning_node::node::FinalTypes;
use lightning_pool::pool::{ConnectionPool, PoolConfig};
use lightning_rpc::config::Config as RpcConfig;
use lightning_rpc::server::Rpc;
use lightning_signer::{utils, Config as SignerConfig, Signer};
use resolved_pathbuf::ResolvedPathBuf;

use crate::containerized_node::ContainerizedNode;
use crate::utils::networking::{PortAssigner, Transport};

pub struct Swarm {
    nodes: HashMap<NodePublicKey, ContainerizedNode>,
    directory: ResolvedPathBuf,
}

impl Drop for Swarm {
    fn drop(&mut self) {
        self.nodes.values_mut().for_each(|node| node.shutdown());
        fs::remove_dir_all(&self.directory).expect("Failed to clean up swarm directory.");
    }
}

impl Swarm {
    pub fn builder() -> SwarmBuilder {
        SwarmBuilder::default()
    }

    pub async fn launch(&self) -> anyhow::Result<()> {
        try_join_all(self.nodes.values().map(|node| node.start())).await?;
        Ok(())
    }

    pub fn get_rpc_addresses(&self) -> HashMap<NodePublicKey, String> {
        self.nodes
            .iter()
            .map(|(pubkey, node)| (*pubkey, node.get_rpc_address()))
            .collect()
    }
}

#[derive(Default)]
pub struct SwarmBuilder {
    directory: Option<ResolvedPathBuf>,
    min_port: Option<u16>,
    max_port: Option<u16>,
    num_nodes: Option<usize>,
    epoch_start: Option<u64>,
    epoch_time: Option<u64>,
    port_assigner: Option<PortAssigner>,
    bootstrappers: Option<Vec<Bootstrapper>>,
    committee_size: Option<u64>,
}

impl SwarmBuilder {
    pub fn with_directory(mut self, directory: ResolvedPathBuf) -> Self {
        self.directory = Some(directory);
        self
    }

    pub fn with_num_nodes(mut self, num_nodes: usize) -> Self {
        self.num_nodes = Some(num_nodes);
        self
    }

    pub fn with_epoch_start(mut self, epoch_start: u64) -> Self {
        self.epoch_start = Some(epoch_start);
        self
    }

    pub fn with_epoch_time(mut self, epoch_time: u64) -> Self {
        self.epoch_time = Some(epoch_time);
        self
    }

    pub fn with_port_assigner(mut self, port_assigner: PortAssigner) -> Self {
        self.port_assigner = Some(port_assigner);
        self
    }

    pub fn with_bootstrappers(mut self, bootstrappers: Vec<Bootstrapper>) -> Self {
        self.bootstrappers = Some(bootstrappers);
        self
    }

    pub fn with_committee_size(mut self, committee_size: u64) -> Self {
        self.committee_size = Some(committee_size);
        self
    }

    pub fn with_min_port(mut self, port: u16) -> Self {
        self.min_port = Some(port);
        self
    }

    pub fn with_max_port(mut self, port: u16) -> Self {
        self.max_port = Some(port);
        self
    }

    pub fn build(self) -> Swarm {
        let num_nodes = self.num_nodes.expect("Number of nodes must be provided.");
        let directory = self.directory.expect("Directory must be provided.");
        let min_port = self.min_port.expect("Minimum port must be provided.");
        let max_port = self.max_port.expect("Maximum port must be provided.");

        let bootstrappers = self.bootstrappers.unwrap_or(Vec::new());
        let mut port_assigner = self.port_assigner.unwrap_or_default();

        // Load the default genesis. Clear the committee and node info and overwrite
        // the provided values from config.
        let mut genesis = Genesis::load().unwrap();
        genesis.committee = Vec::new();
        genesis.node_info = HashMap::new();
        genesis.epoch_start = self.epoch_start.unwrap_or(genesis.epoch_start);
        genesis.epoch_time = self.epoch_time.unwrap_or(genesis.epoch_time);
        genesis.committee_size = self.committee_size.unwrap_or(genesis.committee_size);

        // Make sure the test directory exists by recursively creating it.
        fs::create_dir_all(&directory).expect("Failed to create swarm directory");

        // For the number of nodes that we need. Create the distinct configuration objects which
        // we can pass to the containerized nodes.
        let mut tmp_nodes = Vec::with_capacity(num_nodes);

        for index in 0..num_nodes {
            let root = directory.join(format!("node-{index}"));
            fs::create_dir_all(&root).expect("Failed to create node directory");

            let config = build_config(
                &root,
                |t| {
                    port_assigner
                        .get_port(min_port, max_port, t)
                        .expect("Could not get the port.")
                },
                bootstrappers.clone(),
            );

            // Generate and store the node public key.
            let (node_pk, consensus_pk) = generate_and_store_node_secret(&config);
            let owner_sk = AccountOwnerSecretKey::generate();
            let owner_pk = owner_sk.to_pk();
            let owner_eth: EthAddress = owner_pk.into();

            // Make the node info using the generated config.
            let consensus_config = config.get::<Consensus<FinalTypes>>();

            let worker = Worker {
                public_key: node_pk,
                address: consensus_config.worker_address.clone(),
                mempool: consensus_config.mempool_address.clone(),
            };

            let node_info = NodeInfo {
                owner: owner_eth,
                public_key: node_pk,
                consensus_key: consensus_pk,
                staked_since: 0,
                stake: Staking {
                    staked: HpUfixed::<18>::from(genesis.min_stake),
                    ..Default::default()
                },
                domain: consensus_config.address.clone(),
                workers: vec![worker],
                nonce: 0,
            };

            if (index as u64) < genesis.committee_size {
                genesis.committee.push(node_info.into());
            } else {
                genesis.node_info.insert(node_pk.to_base64(), node_info);
            }

            tmp_nodes.push((owner_sk, node_pk, config));
        }

        // Now that we have built the configuration of all nodes and also have compiled the
        // proper genesis config. We can inject the genesis config.

        let mut nodes = HashMap::new();
        for (index, (owner_sk, node_pk, config)) in tmp_nodes.into_iter().enumerate() {
            config.inject::<Application<FinalTypes>>(AppConfig {
                mode: Mode::Test,
                genesis: Some(genesis.clone()),
            });

            let node = ContainerizedNode::new(config, owner_sk, index);
            nodes.insert(node_pk, node);
        }

        Swarm { nodes, directory }
    }
}

/// Build the configuration object for a
fn build_config<P>(
    root: &Path,
    mut get_port: P,
    bootstrappers: Vec<Bootstrapper>,
) -> TomlConfigProvider<FinalTypes>
where
    P: FnMut(Transport) -> u16,
{
    let config = TomlConfigProvider::<FinalTypes>::default();

    config.inject::<Rpc<FinalTypes>>(RpcConfig {
        port: get_port(Transport::Tcp),
        ..Default::default()
    });

    config.inject::<Consensus<FinalTypes>>(ConsensusConfig {
        address: format!("/ip4/127.0.0.1/udp/{}", get_port(Transport::Udp))
            .parse()
            .unwrap(),
        worker_address: format!("/ip4/127.0.0.1/udp/{}/http", get_port(Transport::Udp))
            .parse()
            .unwrap(),
        mempool_address: format!("/ip4/127.0.0.1/tcp/{}/http", get_port(Transport::Tcp))
            .parse()
            .unwrap(),
        store_path: root
            .join("data/narwhal_store")
            .try_into()
            .expect("Failed to resolve path"),
    });

    config.inject::<Dht<FinalTypes>>(DhtConfig {
        address: format!("127.0.0.1:{}", get_port(Transport::Udp))
            .parse()
            .unwrap(),
        bootstrappers,
    });

    config.inject::<TcpHandshakeServer<FinalTypes>>(HandshakeServerConfig {
        listen_addr: SocketAddr::from_str(&format!("127.0.0.1:{}", get_port(Transport::Tcp)))
            .unwrap(),
    });

    config.inject::<Signer<FinalTypes>>(SignerConfig {
        node_key_path: root
            .join("keys/node.pem")
            .try_into()
            .expect("Failed to resolve path"),
        consensus_key_path: root
            .join("keys/consensus.pem")
            .try_into()
            .expect("Failed to resolve path"),
    });

    config.inject::<ConnectionPool<FinalTypes>>(PoolConfig {
        address: format!("127.0.0.1:{}", get_port(Transport::Udp))
            .parse()
            .unwrap(),
    });

    config
}

/// Given the configuration of a node, generate and store the networking and consensus secret keys
/// of the node and write them into the path specified by the configuration of the signer.
///
/// Returns the public keys of the generated keys.
fn generate_and_store_node_secret(
    config: &TomlConfigProvider<FinalTypes>,
) -> (NodePublicKey, ConsensusPublicKey) {
    let config = config.get::<Signer<FinalTypes>>();

    let node_secret_key = NodeSecretKey::generate();
    let node_consensus_secret_key = ConsensusSecretKey::generate();

    utils::save(&config.node_key_path, node_secret_key.encode_pem())
        .expect("Failed to save node secret key.");

    utils::save(
        &config.consensus_key_path,
        node_consensus_secret_key.encode_pem(),
    )
    .expect("Failed to save consensus secret key.");

    (node_secret_key.to_pk(), node_consensus_secret_key.to_pk())
}
