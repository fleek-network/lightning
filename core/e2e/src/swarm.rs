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
use lightning_handshake::server::{HandshakeServer, HandshakeServerConfig, TcpProvider};
use lightning_interfaces::{BroadcastInterface, ConfigConsumer};
use lightning_node::{config::TomlConfigProvider, template::broadcast::Broadcast};
use lightning_notifier::Notifier;
use lightning_rpc::{config::Config as RpcConfig, server::Rpc};
use lightning_signer::{utils, Config as SignerConfig, Signer};
use lightning_topology::Topology;
use toml::Value;

use crate::containerized_node::ContainerizedNode;

type MyBroadcast = Broadcast<QueryRunner, Signer, Topology<QueryRunner>, Notifier>;

pub struct Swarm {
    nodes: HashMap<NodePublicKey, ContainerizedNode>,
    directory: PathBuf,
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
    directory: Option<PathBuf>,
    num_nodes: Option<usize>,
    epoch_start: Option<u64>,
    epoch_time: Option<u64>,
}

impl SwarmBuilder {
    pub fn with_directory(mut self, directory: PathBuf) -> Self {
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

        let min_stake = genesis.min_stake;
        genesis.committee = Vec::new();
        for i in 0..num_nodes {
            let node_directory = directory.join(format!("swarm/nodes/{i}"));
            fs::create_dir_all(&directory).expect("Failed to create node directory");

            let config = TomlConfigProvider::default();

            let mut rpc_config = RpcConfig::default();
            rpc_config.port += i as u16;
            config.table.lock().expect("Failed to aqcuire lock").insert(
                Rpc::<QueryRunner>::KEY.into(),
                Value::try_from(&rpc_config).unwrap(),
            );

            let consensus_config = ConsensusConfig {
                address: format!("/ip4/0.0.0.0/udp/{}", 8000 + i).parse().unwrap(),
                worker_address: format!("/ip4/0.0.0.0/udp/{}", 8000 + num_nodes + i)
                    .parse()
                    .unwrap(),
                mempool_address: format!("/ip4/0.0.0.0/udp/{}", 8000 + (2 * num_nodes) + i)
                    .parse()
                    .unwrap(),
                store_path: node_directory.join("data/narwhal_store"),
            };

            config.table.lock().expect("Failed to aqcuire lock").insert(
                Consensus::<
                    QueryRunner,
                    <MyBroadcast as BroadcastInterface>::PubSub<PubSubMsg>
                >::KEY
                    .into(),
                Value::try_from(&consensus_config).unwrap(),
            );

            let handshake_config = HandshakeServerConfig {
                listen_addr: SocketAddr::from_str(&format!("0.0.0.0:{}", 6969 + i)).unwrap(),
            };

            let keys_path = node_directory.join("keys");
            let owner_secret_key = AccountOwnerSecretKey::generate();
            let node_secret_key = NodeSecretKey::generate();
            let node_secret_key_path = keys_path.join("node.pem");
            utils::save(&node_secret_key_path, node_secret_key.encode_pem())
                .expect("Failed to save node secret key.");
            let node_net_secret_key = NodeNetworkingSecretKey::generate();
            let node_net_secret_key_path = keys_path.join("network.pem");
            utils::save(&node_net_secret_key_path, node_net_secret_key.encode_pem())
                .expect("Failed to save node secret key.");
            let signer_config = SignerConfig {
                node_key_path: node_secret_key_path,
                network_key_path: node_net_secret_key_path,
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
                format!("/ip4/127.0.0.1/udp/{}", 8000 + i),
                node_net_secret_key.to_pk().to_base64(),
                format!("/ip4/127.0.0.1/udp/{}/http", 8000 + num_nodes + i),
                node_net_secret_key.to_pk().to_base64(),
                format!("/ip4/127.0.0.1/tcp/{}/http", 8000 + (2 * num_nodes) + i),
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
