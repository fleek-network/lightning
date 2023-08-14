use std::{
    borrow::BorrowMut, collections::HashMap, fs, net::SocketAddr, path::PathBuf, str::FromStr,
    sync::Arc,
};

use fleek_crypto::{
    AccountOwnerSecretKey, NodeNetworkingSecretKey, NodePublicKey, NodeSecretKey, PublicKey,
    SecretKey,
};
use futures::future::try_join_all;
use lightning_application::{
    app::Application,
    config::{Config as AppConfig, Mode},
    genesis::{Genesis, GenesisCommittee},
    query_runner::QueryRunner,
};
use lightning_consensus::{
    config::Config as ConsensusConfig,
    consensus::{Consensus, PubSubMsg},
};
use lightning_dht::{
    config::{Bootstrapper, Config as DhtConfig},
    dht::Dht,
};
use lightning_handshake::server::{HandshakeServer, HandshakeServerConfig, TcpProvider};
use lightning_interfaces::{BroadcastInterface, ConfigConsumer};
use lightning_node::{config::TomlConfigProvider, template::broadcast::Broadcast};
use lightning_notifier::Notifier;
use lightning_rpc::{config::Config as RpcConfig, server::Rpc};
use lightning_signer::{utils, Config as SignerConfig, Signer};
use lightning_topology::Topology;
use resolved_pathbuf::ResolvedPathBuf;
use toml::Value;

use crate::{
    containerized_node::ContainerizedNode,
    utils::networking::{PortAssigner, Transport},
};

type MyBroadcast = Broadcast<QueryRunner, Signer, Topology<QueryRunner>, Notifier>;

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
    num_nodes: Option<usize>,
    epoch_start: Option<u64>,
    epoch_time: Option<u64>,
    port_assigner: Option<PortAssigner>,
    bootstrappers: Option<Vec<Bootstrapper>>,
}

impl SwarmBuilder {
    pub fn with_directory(mut self, directory: PathBuf) -> Self {
        self.directory = Some(directory.try_into().expect("Failed to resolve path"));
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

    pub fn build(self) -> Swarm {
        let num_nodes = self.num_nodes.expect("Number of nodes must be provided.");
        let directory = self.directory.expect("Directory must be provided.");

        let mut genesis = Genesis::load().unwrap();
        if let Some(epoch_start) = self.epoch_start {
            genesis.epoch_start = epoch_start;
        }
        if let Some(epoch_time) = self.epoch_time {
            genesis.epoch_time = epoch_time;
        }

        let mut nodes = HashMap::with_capacity(num_nodes);

        fs::create_dir_all(&directory).expect("Failed to create swarm directory");

        let mut port_assigner = match self.port_assigner {
            Some(port_assigner) => port_assigner,
            None => PortAssigner::default(),
        };

        let bootstrappers = self.bootstrappers.unwrap_or(Vec::new());

        let min_stake = genesis.min_stake;
        genesis.committee = Vec::new();
        for i in 0..num_nodes {
            let primary_addr_port = port_assigner
                .get_port(8000, 40000, Transport::Udp)
                .expect("Failed to get available port.");
            let worker_addr_port = port_assigner
                .get_port(8000, 40000, Transport::Udp)
                .expect("Failed to get available port.");
            let mempool_addr_port = port_assigner
                .get_port(8000, 40000, Transport::Tcp)
                .expect("Failed to get available port.");
            let dht_port = port_assigner
                .get_port(8000, 40000, Transport::Udp)
                .expect("Failed to get available port.");

            let node_directory = directory.join(format!("swarm/nodes/{i}"));
            fs::create_dir_all(&directory).expect("Failed to create node directory");

            let config = TomlConfigProvider::default();

            let mut rpc_config = RpcConfig::default();
            rpc_config.port += i as u16;
            config.table.lock().expect("Failed to aqcuire lock").insert(
                Rpc::<QueryRunner>::KEY.into(),
                Value::try_from(&rpc_config).unwrap(),
            );

            let dht_config = DhtConfig {
                address: format!("0.0.0.0:{dht_port}").parse().unwrap(),
                bootstrappers: bootstrappers.clone(),
            };
            config.table.lock().expect("Failed to aqcuire lock").insert(
                Dht::<Topology<QueryRunner>>::KEY.into(),
                Value::try_from(&dht_config).unwrap(),
            );

            let consensus_config = ConsensusConfig {
                address: format!("/ip4/0.0.0.0/udp/{primary_addr_port}")
                    .parse()
                    .unwrap(),
                worker_address: format!("/ip4/0.0.0.0/udp/{worker_addr_port}")
                    .parse()
                    .unwrap(),
                mempool_address: format!("/ip4/0.0.0.0/tcp/{mempool_addr_port}")
                    .parse()
                    .unwrap(),
                store_path: node_directory
                    .join("data/narwhal_store")
                    .try_into()
                    .expect("Failed to resolve path"),
            };

            config.table.lock().expect("Failed to aqcuire lock").insert(
                Consensus::<
                    QueryRunner,
                    <MyBroadcast as BroadcastInterface>::PubSub<PubSubMsg>
                >::KEY
                    .into(),
                Value::try_from(&consensus_config).unwrap(),
            );

            let handshake_addr_port = port_assigner
                .get_port(6969, 40000, Transport::Tcp)
                .expect("Failed to get available port.");
            let handshake_config = HandshakeServerConfig {
                listen_addr: SocketAddr::from_str(&format!("0.0.0.0:{handshake_addr_port}"))
                    .unwrap(),
            };

            let keys_path = node_directory.join("keys");
            let owner_secret_key_path = keys_path.join("account.pem");
            let owner_secret_key = AccountOwnerSecretKey::generate();
            utils::save(&owner_secret_key_path, owner_secret_key.encode_pem())
                .expect("Failed to save account owner secret key.");
            let node_secret_key = NodeSecretKey::generate();
            let node_secret_key_path = keys_path.join("node.pem");
            utils::save(&node_secret_key_path, node_secret_key.encode_pem())
                .expect("Failed to save node secret key.");
            let node_net_secret_key = NodeNetworkingSecretKey::generate();
            let node_net_secret_key_path = keys_path.join("network.pem");
            utils::save(&node_net_secret_key_path, node_net_secret_key.encode_pem())
                .expect("Failed to save node secret key.");
            let signer_config = SignerConfig {
                node_key_path: node_secret_key_path
                    .try_into()
                    .expect("Failed to resolve path"),
                network_key_path: node_net_secret_key_path
                    .try_into()
                    .expect("Failed to resolve path"),
            };

            config
                .table
                .lock()
                .expect("Failed to aqcurie lock")
                .insert(Signer::KEY.into(), Value::try_from(&signer_config).unwrap());

            config.table.lock().expect("Failed to aqcuire lock").insert(
                HandshakeServer::<TcpProvider>::KEY.into(),
                Value::try_from(&handshake_config).unwrap(),
            );

            genesis.borrow_mut().committee.push(GenesisCommittee::new(
                owner_secret_key.to_pk().to_base64(),
                node_secret_key.to_pk().to_base64(),
                format!("/ip4/127.0.0.1/udp/{primary_addr_port}"),
                node_net_secret_key.to_pk().to_base64(),
                format!("/ip4/127.0.0.1/udp/{worker_addr_port}/http"),
                node_net_secret_key.to_pk().to_base64(),
                format!("/ip4/127.0.0.1/tcp/{mempool_addr_port}/http"),
                Some(min_stake),
            ));

            nodes.insert(node_secret_key.to_pk(), (config, owner_secret_key, i));
        }

        let nodes = nodes
            .into_iter()
            .map(|(node_pub_key, (config, owner_secret_key, index))| {
                let app_config = AppConfig {
                    mode: Mode::Test,
                    genesis: Some(genesis.clone()),
                };

                config.table.lock().expect("Failed to aqcuire lock").insert(
                    Application::KEY.into(),
                    Value::try_from(&app_config).unwrap(),
                );

                (
                    node_pub_key,
                    ContainerizedNode::new(Arc::new(config), owner_secret_key, index),
                )
            })
            .collect();

        Swarm { nodes, directory }
    }
}
