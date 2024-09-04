use std::fs::read_to_string;
use std::time::Duration;

use anyhow::{Context, Result};
use atomo::storage::StorageBackend;
use fleek_blake3 as blake3;
use fleek_crypto::{AccountOwnerSecretKey, ConsensusSecretKey, NodeSecretKey, SecretKey};
use lightning_application::app::Application;
use lightning_application::config::{Config as AppConfig, StorageConfig};
use lightning_application::env::Env;
use lightning_application::genesis::{Genesis, GenesisNode};
use lightning_archive::archive::Archive;
use lightning_archive::config::Config as ArchiveConfig;
use lightning_blockstore::blockstore::Blockstore;
use lightning_blockstore::config::Config as BlockstoreConfig;
use lightning_blockstore_server::{BlockstoreServer, Config as BlockstoreServerConfig};
use lightning_consensus::config::Config as ConsensusConfig;
use lightning_consensus::consensus::Consensus;
use lightning_dack_aggregator::{
    Config as DeliveryAcknowledgmentConfig,
    DeliveryAcknowledgmentAggregator,
};
use lightning_final_bindings::FinalTypes;
use lightning_handshake::config::{HandshakeConfig, TransportConfig};
use lightning_handshake::handshake::Handshake;
use lightning_handshake::transports::webrtc::WebRtcConfig;
use lightning_interfaces::prelude::*;
use lightning_interfaces::types::{HandshakePorts, NodePorts};
use lightning_keystore::{Keystore, KeystoreConfig};
use lightning_pinger::{Config as PingerConfig, Pinger};
use lightning_pool::{Config as PoolConfig, PoolProvider};
use lightning_rep_collector::config::Config as RepAggConfig;
use lightning_rep_collector::ReputationAggregator;
use lightning_resolver::config::Config as ResolverConfig;
use lightning_resolver::resolver::Resolver;
use lightning_rpc::{Config as RpcConfig, Rpc};
use lightning_service_executor::shim::{ServiceExecutor, ServiceExecutorConfig};
use lightning_syncronizer::config::Config as SyncronizerConfig;
use lightning_syncronizer::syncronizer::Syncronizer;
use lightning_utils::config::TomlConfigProvider;
use lightning_utils::shutdown::ShutdownController;
use resolved_pathbuf::ResolvedPathBuf;
use serial_test::serial;
use tempfile::{tempdir, TempDir};
use tokio::pin;

const NUM_RESTARTS: u16 = 3;

fn build_config(
    temp_dir: &TempDir,
    genesis_path: ResolvedPathBuf,
    keystore_config: KeystoreConfig,
    ports: NodePorts,
) -> TomlConfigProvider<FinalTypes> {
    let path = temp_dir.path().to_path_buf();

    let config = TomlConfigProvider::<FinalTypes>::default();

    config.inject::<Application<FinalTypes>>(AppConfig {
        network: None,
        genesis_path: Some(genesis_path),
        storage: StorageConfig::RocksDb,
        db_path: Some(path.join("data/app_db").try_into().unwrap()),
        db_options: None,
        dev: None,
    });

    config.inject::<Resolver<FinalTypes>>(ResolverConfig {
        store_path: path
            .join("data/resolver_store")
            .try_into()
            .expect("Failed to resolve path"),
    });
    config.inject::<Rpc<FinalTypes>>(RpcConfig {
        hmac_secret_dir: Some(temp_dir.path().to_path_buf()),
        ..RpcConfig::default_with_port(ports.rpc)
    });

    config.inject::<Consensus<FinalTypes>>(ConsensusConfig {
        store_path: path
            .join("data/narwhal_store")
            .try_into()
            .expect("Failed to resolve path"),
    });

    //config.inject::<Signer<FinalTypes>>(SignerConfig {
    //    node_key_path: path
    //        .join("keys/node.pem")
    //        .try_into()
    //        .expect("Failed to resolve path"),
    //    consensus_key_path: path
    //        .join("keys/consensus.pem")
    //        .try_into()
    //        .expect("Failed to resolve path"),
    //});

    config.inject::<Keystore<FinalTypes>>(keystore_config);

    config.inject::<Blockstore<FinalTypes>>(BlockstoreConfig {
        root: path
            .join("data/blockstore")
            .try_into()
            .expect("Failed to resolve path"),
    });

    config.inject::<BlockstoreServer<FinalTypes>>(BlockstoreServerConfig::default());

    config.inject::<Handshake<FinalTypes>>(HandshakeConfig {
        // TODO: figure out how to have e2e testing for the different transports (browser oriented)
        transports: vec![TransportConfig::WebRTC(WebRtcConfig {
            address: ([0, 0, 0, 0], ports.handshake.webrtc).into(),
        })],
        http_address: ([127, 0, 0, 1], ports.handshake.http).into(),
        ..Default::default()
    });

    config.inject::<ServiceExecutor<FinalTypes>>(ServiceExecutorConfig {
        services: Default::default(),
        ..Default::default()
    });

    config.inject::<ReputationAggregator<FinalTypes>>(RepAggConfig {
        reporter_buffer_size: 1,
    });

    config.inject::<PoolProvider<FinalTypes>>(PoolConfig {
        address: format!("127.0.0.1:{}", ports.pool).parse().unwrap(),
        ..Default::default()
    });

    config.inject::<Syncronizer<FinalTypes>>(SyncronizerConfig {
        epoch_change_delta: Duration::from_secs(500),
    });

    config.inject::<Archive<FinalTypes>>(ArchiveConfig {
        is_archive: true,
        store_path: path
            .join("data/archive")
            .try_into()
            .expect("Failed to resolve path"),
    });

    config.inject::<Pinger<FinalTypes>>(PingerConfig {
        address: format!("127.0.0.1:{}", ports.pinger).parse().unwrap(),
        ping_interval: Duration::from_secs(5),
    });

    config.inject::<DeliveryAcknowledgmentAggregator<FinalTypes>>(DeliveryAcknowledgmentConfig {
        db_path: path
            .join("data/dack_aggregator")
            .try_into()
            .expect("Failed to resolve path"),
        ..Default::default()
    });

    config
}

#[tokio::test]
#[serial]
async fn node_checkpointing() -> Result<()> {
    let temp_dir = tempdir()?;

    let shutdown_controller = ShutdownController::default();
    shutdown_controller.install_handlers();

    let signer_config = KeystoreConfig::test();
    let node_secret_key =
        read_to_string(&signer_config.node_key_path).context("Failed to read node pem file")?;
    let node_secret_key =
        NodeSecretKey::decode_pem(&node_secret_key).context("Failed to decode node pem file")?;
    let consensus_secret_key = read_to_string(&signer_config.consensus_key_path)
        .context("Failed to read consensus pem file")?;
    let consensus_secret_key = ConsensusSecretKey::decode_pem(&consensus_secret_key)
        .context("Failed to decode consensus pem file")?;

    let node_public_key = node_secret_key.to_pk();
    let consensus_public_key = consensus_secret_key.to_pk();
    let owner_public_key = AccountOwnerSecretKey::generate().to_pk();

    let mut genesis = Genesis {
        committee_size: 10,
        node_count: 100,
        min_stake: 1000,

        ..Genesis::default()
    };

    let node_ports = NodePorts {
        primary: 30100,
        worker: 30101,
        mempool: 30102,
        rpc: 30103,
        pool: 30104,
        pinger: 30105,
        handshake: HandshakePorts {
            http: 30106,
            webrtc: 30107,
            webtransport: 30108,
        },
    };

    let genesis_node = GenesisNode::new(
        owner_public_key.into(),
        node_public_key,
        "127.0.0.1".parse().unwrap(),
        consensus_public_key,
        "127.0.0.1".parse().unwrap(),
        node_public_key,
        node_ports.clone(),
        None,
        true,
    );

    genesis.node_info.push(genesis_node);

    let genesis_path = genesis
        .write_to_dir(temp_dir.path().to_path_buf().try_into().unwrap())
        .unwrap();

    // We first have to build the app db in order to obtain a valid checkpoint.
    let app_config_temp = AppConfig {
        network: None,
        genesis_path: Some(genesis_path.clone()),
        storage: StorageConfig::RocksDb,
        db_path: Some(temp_dir.path().join("data/app_db_temp").try_into().unwrap()),
        db_options: None,
        dev: None,
    };
    let mut env = Env::new(&app_config_temp, None)?;
    env.apply_genesis_block(&app_config_temp)?;

    let storage = env.inner.get_storage_backend_unsafe();
    let checkpoint = storage.serialize().unwrap();
    let checkpoint_hash = blake3::hash(&checkpoint);
    std::mem::drop(env);

    // Now that we have a checkpoint, we initialize the node.
    let config = build_config(&temp_dir, genesis_path, signer_config, node_ports);
    let app_config = config.get::<<FinalTypes as Collection>::ApplicationInterface>();

    let mut node = Node::<FinalTypes>::init(config.clone())
        .map_err(|e| anyhow::anyhow!("Node Initialization failed: {e:?}"))
        .context("Could not start the node.")?;

    node.start().await;

    let shutdown_future = shutdown_controller.wait_for_shutdown();
    pin!(shutdown_future);

    let mut interval = tokio::time::interval(Duration::from_secs(30));

    let mut count = 0;
    loop {
        tokio::select! {
            _ = &mut shutdown_future => break,
            _ = interval.tick() => {

                // wait for node to start
                tokio::time::sleep(Duration::from_secs(30)).await;

                // shutdown the node
                node.shutdown().await;

                std::mem::drop(node);

                // start local env in checkpoint mode to seed database with the new checkpoint
                <FinalTypes as Collection>::ApplicationInterface::load_from_checkpoint(
                    &app_config, checkpoint.clone(), *checkpoint_hash.as_bytes()).await?;

                node = Node::<FinalTypes>::init(config.clone())
                    .map_err(|e| anyhow::anyhow!("Could not start the node: {e:?}"))?;

                node.start().await;

                count += 1;
                if count > NUM_RESTARTS {
                    break;
                }
            }
        }
    }

    tokio::time::sleep(Duration::from_secs(30)).await;
    node.shutdown().await;

    Ok(())
}
