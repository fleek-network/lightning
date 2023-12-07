use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::time::Duration;

use fleek_crypto::{
    AccountOwnerSecretKey,
    ConsensusPublicKey,
    ConsensusSecretKey,
    EthAddress,
    NodePublicKey,
    NodeSecretKey,
    SecretKey,
};
use futures::future::try_join_all;
use hp_fixed::unsigned::HpUfixed;
use lightning_application::app::Application;
use lightning_application::config::{Config as AppConfig, Mode, StorageConfig};
use lightning_application::genesis::{Genesis, GenesisNode};
use lightning_archive::archive::Archive;
use lightning_archive::config::Config as ArchiveConfig;
use lightning_blockstore::blockstore::Blockstore;
use lightning_blockstore::config::Config as BlockstoreConfig;
use lightning_blockstore_server::{BlockStoreServer, Config as BlockStoreServerConfig};
use lightning_broadcast::{Broadcast, Config as BroadcastConfig};
use lightning_consensus::config::Config as ConsensusConfig;
use lightning_consensus::consensus::Consensus;
use lightning_dht::config::Config as DhtConfig;
use lightning_dht::Dht;
use lightning_handshake::config::{HandshakeConfig, TransportConfig};
use lightning_handshake::handshake::Handshake;
use lightning_handshake::transports::webrtc::WebRtcConfig;
use lightning_interfaces::types::{Blake3Hash, NodePorts, Staking};
use lightning_interfaces::ConfigProviderInterface;
use lightning_node::config::TomlConfigProvider;
use lightning_node::FinalTypes;
use lightning_pinger::{Config as PingerConfig, Pinger};
use lightning_pool::{Config as PoolConfig, Pool};
use lightning_rep_collector::config::Config as RepAggConfig;
use lightning_rep_collector::ReputationAggregator;
use lightning_resolver::config::Config as ResolverConfig;
use lightning_resolver::resolver::Resolver;
use lightning_rpc::config::Config as RpcConfig;
use lightning_rpc::Rpc;
use lightning_service_executor::shim::{ServiceExecutor, ServiceExecutorConfig};
use lightning_signer::{utils, Config as SignerConfig, Signer};
use lightning_syncronizer::config::Config as SyncronizerConfig;
use lightning_syncronizer::syncronizer::Syncronizer;
use resolved_pathbuf::ResolvedPathBuf;
use tokio::sync::oneshot;

use crate::containerized_node::ContainerizedNode;
use crate::utils::networking::{PortAssigner, Transport};

pub struct Swarm {
    nodes: HashMap<NodePublicKey, ContainerizedNode>,
    directory: ResolvedPathBuf,
}

impl Drop for Swarm {
    fn drop(&mut self) {
        self.shutdown_internal();
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

    pub async fn launch_genesis_committee(&self) -> anyhow::Result<()> {
        try_join_all(
            self.nodes
                .values()
                .filter(|node| node.is_genesis_committee())
                .map(|node| node.start()),
        )
        .await?;
        Ok(())
    }

    pub async fn launch_non_genesis_committee(&self) -> anyhow::Result<()> {
        try_join_all(
            self.nodes
                .values()
                .filter(|node| !node.is_genesis_committee())
                .map(|node| node.start()),
        )
        .await?;
        Ok(())
    }

    pub async fn launch_bootstrap_nodes(&self) -> anyhow::Result<()> {
        try_join_all(
            self.nodes
                .values()
                .filter(|node| node.is_bootstrap_node())
                .map(|node| node.start()),
        )
        .await?;
        Ok(())
    }

    pub async fn launch_non_bootstrap_nodes(&self) -> anyhow::Result<()> {
        try_join_all(
            self.nodes
                .values()
                .filter(|node| !node.is_bootstrap_node())
                .map(|node| node.start()),
        )
        .await?;
        Ok(())
    }

    pub fn shutdown(mut self) {
        self.shutdown_internal();
    }

    pub fn get_rpc_addresses(&self) -> HashMap<NodePublicKey, String> {
        self.nodes
            .iter()
            .map(|(pubkey, node)| (*pubkey, node.get_rpc_address()))
            .collect()
    }

    pub fn get_genesis_committee_rpc_addresses(&self) -> HashMap<NodePublicKey, String> {
        self.nodes
            .iter()
            .filter(|(_, node)| node.is_genesis_committee())
            .map(|(pubkey, node)| (*pubkey, node.get_rpc_address()))
            .collect()
    }

    pub fn get_non_genesis_committee_rpc_addresses(&self) -> HashMap<NodePublicKey, String> {
        self.nodes
            .iter()
            .filter(|(_, node)| !node.is_genesis_committee())
            .map(|(pubkey, node)| (*pubkey, node.get_rpc_address()))
            .collect()
    }

    pub fn get_non_genesis_committee_ckpt_rx(
        &self,
    ) -> Vec<(NodePublicKey, Option<oneshot::Receiver<Blake3Hash>>)> {
        self.nodes
            .iter()
            .filter(|(_pubkey, node)| !node.is_genesis_committee())
            .map(|(pubkey, node)| (*pubkey, node.take_ckpt_rx()))
            .collect()
    }

    pub fn get_dht_handle(&self) -> Vec<Option<Dht<FinalTypes>>> {
        self.nodes
            .values()
            .filter(|node| !node.is_bootstrap_node())
            .map(|node| node.take_dht_socket())
            .collect()
    }

    pub fn get_blockstores(&self) -> Vec<Option<Blockstore<FinalTypes>>> {
        self.nodes
            .values()
            .map(|node| node.take_blockstore())
            .collect()
    }

    pub fn get_blockstore(&self, node: &NodePublicKey) -> Option<Blockstore<FinalTypes>> {
        self.nodes.get(node).and_then(|node| node.take_blockstore())
    }

    pub fn get_bootstrap_nodes_pk(&self) -> Vec<NodePublicKey> {
        self.nodes
            .iter()
            .filter_map(|(key, node)| {
                if node.is_bootstrap_node() {
                    Some(*key)
                } else {
                    None
                }
            })
            .collect()
    }

    fn shutdown_internal(&mut self) {
        self.nodes.values().for_each(|node| node.shutdown());
        if self.directory.exists() {
            fs::remove_dir_all(&self.directory).expect("Failed to clean up swarm directory.");
        }
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
    syncronizer_delta: Option<Duration>,
    archiver: bool,
    use_persistence: bool,
    committee_size: Option<u64>,
    add_bootstrap_node: bool,
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

    pub fn use_persistence(mut self) -> Self {
        self.use_persistence = true;
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

    pub fn with_archiver(mut self) -> Self {
        self.archiver = true;
        self
    }

    pub fn with_syncronizer_delta(mut self, delta: Duration) -> Self {
        self.syncronizer_delta = Some(delta);
        self
    }

    pub fn with_bootstrap_node(mut self) -> Self {
        self.add_bootstrap_node = true;
        self
    }

    pub fn build(self) -> Swarm {
        let num_nodes = self.num_nodes.expect("Number of nodes must be provided.");
        let directory = self.directory.expect("Directory must be provided.");
        let min_port = self.min_port.expect("Minimum port must be provided.");
        let max_port = self.max_port.expect("Maximum port must be provided.");

        let mut port_assigner = self.port_assigner.unwrap_or_default();

        // Load the default genesis. Clear the committee and node info and overwrite
        // the provided values from config.
        let mut genesis = Genesis::load().unwrap();

        genesis.node_info = Vec::with_capacity(num_nodes);
        genesis.epoch_start = self.epoch_start.unwrap_or(genesis.epoch_start);
        genesis.epoch_time = self.epoch_time.unwrap_or(genesis.epoch_time);
        genesis.committee_size = self.committee_size.unwrap_or(genesis.committee_size);

        // Make sure the test directory exists by recursively creating it.
        fs::create_dir_all(&directory).expect("Failed to create swarm directory");

        // For the number of nodes that we need. Create the distinct configuration objects which
        // we can pass to the containerized nodes.
        let mut tmp_nodes = Vec::with_capacity(num_nodes);

        // If a bootstrap node has to be added, the first node will be assigned and the remaining
        // will be configured to bootstrap to that one.
        // Care must be taken to not take or replace this value once it is set to Some.
        // Todo: make this setup for bootstraps more extensible and explicit.
        let mut bootstrappers: Option<Vec<NodePublicKey>> = None;
        for index in 0..num_nodes {
            let root = directory.join(format!("node-{index}"));
            fs::create_dir_all(&root).expect("Failed to create node directory");

            let ports = NodePorts {
                primary: port_assigner
                    .get_port(min_port, max_port, Transport::Udp)
                    .expect("Could not get port"),
                worker: port_assigner
                    .get_port(min_port, max_port, Transport::Udp)
                    .expect("Could not get port"),
                mempool: port_assigner
                    .get_port(min_port, max_port, Transport::Tcp)
                    .expect("Could not get port"),
                rpc: port_assigner
                    .get_port(min_port, max_port, Transport::Tcp)
                    .expect("Could not get port"),
                pool: port_assigner
                    .get_port(min_port, max_port, Transport::Udp)
                    .expect("Could not get port"),
                dht: port_assigner
                    .get_port(min_port, max_port, Transport::Udp)
                    .expect("Could not get port"),
                pinger: port_assigner
                    .get_port(min_port, max_port, Transport::Udp)
                    .expect("Could not get port"),
                handshake: lightning_interfaces::types::HandshakePorts {
                    http: port_assigner
                        .get_port(min_port, max_port, Transport::Tcp)
                        .expect("Could not get port"),
                    webrtc: port_assigner
                        .get_port(min_port, max_port, Transport::Udp)
                        .expect("Could not get port"),
                    webtransport: port_assigner
                        .get_port(min_port, max_port, Transport::Udp)
                        .expect("Could not get port"),
                },
            };
            let config = build_config(
                &root,
                ports.clone(),
                bootstrappers.clone().unwrap_or(Vec::new()),
                self.archiver,
                self.syncronizer_delta.unwrap_or(Duration::from_secs(300)),
            );

            // Generate and store the node public key.
            let (node_pk, consensus_pk) = generate_and_store_node_secret(&config);
            let owner_sk = AccountOwnerSecretKey::generate();
            let owner_pk = owner_sk.to_pk();
            let owner_eth: EthAddress = owner_pk.into();

            let is_bootstrap_node = self.add_bootstrap_node && bootstrappers.is_none();
            if is_bootstrap_node {
                bootstrappers.replace(vec![node_pk]);
            }

            let is_committee = (index as u64) < genesis.committee_size;

            let node_info = GenesisNode::new(
                owner_eth,
                node_pk,
                "127.0.0.1".parse().unwrap(),
                consensus_pk,
                "127.0.0.1".parse().unwrap(),
                node_pk,
                ports,
                Some(Staking {
                    staked: HpUfixed::<18>::from(genesis.min_stake),
                    ..Default::default()
                }),
                is_committee,
            );
            genesis.node_info.push(node_info);

            tmp_nodes.push((owner_sk, node_pk, config, is_bootstrap_node));
        }

        // Now that we have built the configuration of all nodes and also have compiled the
        // proper genesis config. We can inject the genesis config.

        let mut nodes = HashMap::new();
        for (index, (owner_sk, node_pk, config, is_bootstrap_node)) in
            tmp_nodes.into_iter().enumerate()
        {
            let is_committee = (index as u64) < genesis.committee_size;
            let root = directory.join(format!("node-{index}"));
            let storage = if self.use_persistence {
                StorageConfig::RocksDb
            } else {
                StorageConfig::InMemory
            };
            config.inject::<Application<FinalTypes>>(AppConfig {
                mode: Mode::Test,
                genesis: Some(genesis.clone()),
                testnet: false,
                storage,
                db_path: Some(root.join("data/app_db").try_into().unwrap()),
                db_options: None,
            });

            let node =
                ContainerizedNode::new(config, owner_sk, index, is_committee, is_bootstrap_node);
            nodes.insert(node_pk, node);
        }

        Swarm { nodes, directory }
    }
}

/// Build the configuration object for a
fn build_config(
    root: &Path,
    ports: NodePorts,
    bootstrappers: Vec<NodePublicKey>,
    archiver: bool,
    syncronizer_delta: Duration,
) -> TomlConfigProvider<FinalTypes> {
    let config = TomlConfigProvider::<FinalTypes>::default();

    config.inject::<Resolver<FinalTypes>>(ResolverConfig {
        store_path: root
            .join("data/resolver_store")
            .try_into()
            .expect("Failed to resolve path"),
    });
    config.inject::<Rpc<FinalTypes>>(RpcConfig::default_with_port(ports.rpc));

    config.inject::<Consensus<FinalTypes>>(ConsensusConfig {
        store_path: root
            .join("data/narwhal_store")
            .try_into()
            .expect("Failed to resolve path"),
    });

    config.inject::<Dht<FinalTypes>>(DhtConfig {
        address: format!("127.0.0.1:{}", ports.dht).parse().unwrap(),
        bootstrappers,
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

    config.inject::<Broadcast<FinalTypes>>(BroadcastConfig {});

    config.inject::<Blockstore<FinalTypes>>(BlockstoreConfig {
        root: root
            .join("data/blockstore")
            .try_into()
            .expect("Failed to resolve path"),
    });

    config.inject::<BlockStoreServer<FinalTypes>>(BlockStoreServerConfig::default());

    config.inject::<Handshake<FinalTypes>>(HandshakeConfig {
        // TODO: figure out how to have e2e testing for the different transports (browser oriented)
        transports: vec![TransportConfig::WebRTC(WebRtcConfig {
            address: ([0, 0, 0, 0], ports.handshake.webrtc).into(),
        })],
        http_address: ([127, 0, 0, 1], ports.handshake.http).into(),
    });

    config.inject::<ServiceExecutor<FinalTypes>>(ServiceExecutorConfig {
        services: Default::default(),
        ..Default::default()
    });

    config.inject::<ReputationAggregator<FinalTypes>>(RepAggConfig {
        reporter_buffer_size: 1,
    });

    config.inject::<Pool<FinalTypes>>(PoolConfig {
        address: format!("127.0.0.1:{}", ports.pool).parse().unwrap(),
        ..Default::default()
    });

    config.inject::<Syncronizer<FinalTypes>>(SyncronizerConfig {
        epoch_change_delta: syncronizer_delta,
    });

    config.inject::<Archive<FinalTypes>>(ArchiveConfig {
        is_archive: archiver,
        store_path: root
            .join("data/archive")
            .try_into()
            .expect("Failed to resolve path"),
    });

    config.inject::<Pinger<FinalTypes>>(PingerConfig {
        address: format!("127.0.0.1:{}", ports.pinger).parse().unwrap(),
        ping_interval: Duration::from_millis(1000),
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
