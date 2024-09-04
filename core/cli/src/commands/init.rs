use std::net::SocketAddr;
use std::time::SystemTime;

use anyhow::{bail, Result};
use fleek_crypto::{
    AccountOwnerSecretKey,
    ConsensusSecretKey,
    EthAddress,
    NodeSecretKey,
    SecretKey,
};
use lightning_application::app::Application;
use lightning_application::config::Config as AppConfig;
use lightning_application::network::Network;
use lightning_final_bindings::UseMockConsensus;
use lightning_handshake::config::HandshakeConfig;
use lightning_handshake::handshake::Handshake;
use lightning_interfaces::prelude::*;
use lightning_interfaces::types::{Genesis, GenesisNode};
use lightning_keystore::Keystore;
use lightning_rpc::{Config as RpcConfig, Rpc};
use lightning_test_utils::consensus::MockConsensus;
use lightning_utils::config::TomlConfigProvider;
use resolved_pathbuf::ResolvedPathBuf;
use tracing::{info, warn};
use types::{NodePorts, Staking};

pub async fn exec<C>(
    config_path: ResolvedPathBuf,
    network: Option<Network>,
    no_generate_keys: bool,
    dev: bool,
    force: bool,
    rpc_address: Option<SocketAddr>,
    handshake_http_address: Option<SocketAddr>,
) -> Result<()>
where
    C: Collection<ConfigProviderInterface = TomlConfigProvider<C>>,
{
    // Error if the configuration file already exists.
    if config_path.exists() {
        if !force {
            bail!(
                "Node configuration file already exists at {}",
                config_path.to_str().unwrap()
            );
        } else {
            warn!(
                "Overwriting configuration file at {}",
                config_path.to_str().unwrap()
            );
        }
    }

    // Initialize a new configuration with defaults.
    let config = TomlConfigProvider::<C>::new();
    <C as Collection>::capture_configs(&config);

    // Validate that either network or dev are set, and assign network if dev is set.
    if !dev && network.is_none() {
        bail!("Either --network or --dev must be provided. See --help for details.");
    }

    // Generate keys if requested.
    if !no_generate_keys {
        let keystore_config = config.get::<C::KeystoreInterface>();
        C::KeystoreInterface::generate_keys(keystore_config, true)?;
    }

    // Set network field in the configuration.
    let mut app_config = AppConfig::default();
    if dev {
        if network.is_some() {
            bail!("Cannot specify both --dev and --network");
        }

        app_config.dev = Some(Default::default());

        // Build and write a local devnet genesis configuration.
        let genesis = build_local_devnet_genesis(config.clone())?;
        let config_dir = config_path.parent().unwrap().to_path_buf();
        let genesis_path = genesis.write_to_dir(config_dir.try_into().unwrap())?;
        info!(
            "Genesis configuration written to {}",
            genesis_path.to_str().unwrap()
        );

        app_config.genesis_path = Some(genesis_path);
    } else {
        app_config.network = network;
    }

    // Inject the application config.
    config.inject::<Application<C>>(app_config.clone());

    // Update RPC address in the configuration if given.
    if let Some(addr) = rpc_address {
        config.inject::<Rpc<C>>(RpcConfig {
            addr,
            ..Default::default()
        });
    }

    // Update handshake HTTP address in the configuration if given.
    if let Some(addr) = handshake_http_address {
        config.inject::<Handshake<C>>(HandshakeConfig {
            http_address: addr,
            ..Default::default()
        });
    }

    // Inject the mock consensus config, for configuring `--with-mock-consensus`
    if dev {
        config.inject::<MockConsensus<UseMockConsensus>>(Default::default());
    }

    // Write the configuration file.
    config.write(&config_path)?;
    info!(
        "Node configuration file written to {}",
        config_path.to_str().unwrap()
    );

    Ok(())
}

fn build_local_devnet_genesis<C>(config: TomlConfigProvider<C>) -> Result<Genesis>
where
    C: Collection<ConfigProviderInterface = TomlConfigProvider<C>>,
{
    // Get the node keys from the keystore.
    let keystore = Keystore::<C>::init(&config).expect("failed to load keystore");
    let node_public_key = keystore.get_ed25519_pk();
    let consensus_public_key = keystore.get_bls_pk();

    // Generate account owner key.
    let owner_secret_key = AccountOwnerSecretKey::generate();
    let owner_public_key = owner_secret_key.to_pk();
    let owner_eth_address: EthAddress = owner_public_key.into();

    // Build configurations for the genesis nodes.
    let num_nodes = 4;
    let mut nodes = Vec::with_capacity(num_nodes);

    // Build the node that we will run.
    nodes.push(GenesisNode {
        owner: owner_eth_address,
        primary_public_key: node_public_key,
        consensus_public_key,
        primary_domain: "127.0.0.1".parse()?,
        worker_domain: "127.0.0.1".parse()?,
        worker_public_key: node_public_key,
        ports: NodePorts::default(),
        stake: Staking {
            staked: 2000_u64.into(),
            ..Default::default()
        },
        reputation: None,
        current_epoch_served: None,
        genesis_committee: true,
    });

    // Build other non-reachable nodes to satisfy consensus requirements.
    for i in 2..num_nodes + 1 {
        let node_secret_key = NodeSecretKey::generate();
        let node_public_key = node_secret_key.to_pk();
        let consensus_secret_key = ConsensusSecretKey::generate();
        let consensus_public_key = consensus_secret_key.to_pk();

        nodes.push(GenesisNode {
            owner: owner_eth_address,
            primary_public_key: node_public_key,
            consensus_public_key,
            primary_domain: format!("127.0.0.{}", i).parse()?,
            worker_domain: format!("127.0.0.{}", i).parse()?,
            worker_public_key: node_public_key,
            ports: NodePorts::default(),
            stake: Staking {
                staked: 2000_u64.into(),
                ..Default::default()
            },
            reputation: None,
            current_epoch_served: None,
            genesis_committee: true,
        });
    }

    // Build the genesis configuration.
    let genesis = Genesis {
        chain_id: 1337,
        epoch_start: SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        epoch_time: 60000,
        committee_size: 1,
        node_count: 1,

        min_stake: 1000,
        eligibility_time: 1,
        lock_time: 5,
        node_share: 80,
        service_builder_share: 20,
        max_inflation: 10,
        max_boost: 4,
        max_lock_time: 1460,
        supply_at_genesis: 1000000,
        min_num_measurements: 2,
        node_info: nodes,

        ..Default::default()
    };

    Ok(genesis)
}
