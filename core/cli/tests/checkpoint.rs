use std::fs::read_to_string;
use std::time::Duration;

use anyhow::{Context, Result};
use fleek_crypto::{AccountOwnerSecretKey, ConsensusSecretKey, NodeSecretKey, SecretKey};
use lightning_application::app::Application;
use lightning_application::config::{ApplicationConfig, StorageConfig};
use lightning_application::env::Env;
use lightning_archive::archive::Archive;
use lightning_archive::config::Config as ArchiveConfig;
use lightning_blockstore::blockstore::Blockstore;
use lightning_blockstore::config::Config as BlockstoreConfig;
use lightning_blockstore_server::{BlockstoreServer, Config as BlockstoreServerConfig};
use lightning_checkpointer::{Checkpointer, CheckpointerConfig, CheckpointerDatabaseConfig};
use lightning_committee_beacon::{
    CommitteeBeaconComponent,
    CommitteeBeaconConfig,
    CommitteeBeaconDatabaseConfig,
};
use lightning_consensus::{Consensus, ConsensusConfig};
use lightning_dack_aggregator::{
    Config as DeliveryAcknowledgmentConfig,
    DeliveryAcknowledgmentAggregator,
};
use lightning_handshake::config::{HandshakeConfig, TransportConfig};
use lightning_handshake::handshake::Handshake;
use lightning_handshake::transports::webrtc::WebRtcConfig;
use lightning_interfaces::prelude::*;
use lightning_interfaces::types::{Genesis, GenesisNode, HandshakePorts, NodePorts};
use lightning_keystore::{Keystore, KeystoreConfig};
use lightning_node::Node;
use lightning_node_bindings::FullNodeComponents;
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
use lightning_test_utils::lsof::wait_for_file_to_close;
use lightning_utils::application::QueryRunnerExt;
use lightning_utils::config::TomlConfigProvider;
use merklize::StateRootHash;
use resolved_pathbuf::ResolvedPathBuf;
use tempfile::{tempdir, TempDir};
use types::{Metadata, Value};

#[tokio::test]
async fn test_node_load_checkpoint_start_shutdown_iterations() {
    let temp_dir = tempdir().unwrap();

    // Build the keystore config.
    let keystore_config = KeystoreConfig::test();

    // Build the genesis.
    let genesis = build_genesis(&keystore_config).unwrap();
    let genesis_path = genesis
        .write_to_dir(temp_dir.path().to_path_buf().try_into().unwrap())
        .unwrap();

    // Build a checkpoint.
    let (checkpoint_hash, _, checkpoint) = build_checkpoint(&genesis, &genesis_path).unwrap();

    // Build the node config.
    let config = build_node_config(
        &temp_dir,
        genesis_path,
        keystore_config,
        genesis.node_info[0].ports.clone(),
    );
    let app_config = config.get::<<FullNodeComponents as NodeComponents>::ApplicationInterface>();

    for _ in 0..10 {
        // Load the checkpoint into the application state database.
        <FullNodeComponents as NodeComponents>::ApplicationInterface::load_from_checkpoint(
            &app_config,
            checkpoint.clone(),
            checkpoint_hash,
        )
        .await
        .unwrap();

        // Initialize the node with the same config.
        let mut node = Node::<FullNodeComponents>::init(config.clone()).unwrap();

        // Start the node.
        node.start().await;

        // Shutdown and drop the node.
        node.shutdown().await;
        drop(node);

        // Wait for the database locks to be fully released.
        // This prevents a race condition where the operating system may take time to release
        // file locks or close file handles on the database files, causing the next iteration
        // to fail.
        wait_for_database_locks(&config).await.unwrap();
    }
}

#[tokio::test]
async fn test_node_load_checkpoint_start_ready_shutdown_iterations() {
    let temp_dir = tempdir().unwrap();

    // Build the keystore config.
    let keystore_config = KeystoreConfig::test();

    // Build the genesis.
    let genesis = build_genesis(&keystore_config).unwrap();
    let genesis_path = genesis
        .write_to_dir(temp_dir.path().to_path_buf().try_into().unwrap())
        .unwrap();

    // Build a checkpoint.
    let (checkpoint_hash, checkpoint_state_root, checkpoint) =
        build_checkpoint(&genesis, &genesis_path).unwrap();

    // Build the node config.
    let config = build_node_config(
        &temp_dir,
        genesis_path,
        keystore_config,
        genesis.node_info[0].ports.clone(),
    );
    let app_config = config.get::<<FullNodeComponents as NodeComponents>::ApplicationInterface>();

    for _ in 0..10 {
        // Load the checkpoint into the application state database.
        <FullNodeComponents as NodeComponents>::ApplicationInterface::load_from_checkpoint(
            &app_config,
            checkpoint.clone(),
            checkpoint_hash,
        )
        .await
        .unwrap();

        // Initialize the node with the same config.
        let mut node = Node::<FullNodeComponents>::init(config.clone()).unwrap();

        // Start the node and wait for it to be ready.
        node.start().await;
        node.wait_for_ready(Some(Duration::from_secs(5)))
            .await
            .unwrap();

        // Check the application state.
        {
            let app = node.provider.get::<Application<FullNodeComponents>>();
            let app_query = app.sync_query();

            // Check some entries from the state.
            assert_eq!(app_query.get_chain_id(), genesis.chain_id);

            // Check the last epoch hash. It should be the checkpoint hash.
            let last_epoch_hash = match app_query.get_metadata(&Metadata::LastEpochHash).unwrap() {
                Value::Hash(hash) => hash,
                _ => unreachable!("invalid last epoch hash in metadata"),
            };
            assert_eq!(last_epoch_hash, checkpoint_hash);

            // Check the state tree root.
            // NOTE: We can't check equality here because the `Metadata::LastEpochHash` is not set
            // in the checkpoint but is in the live application state after loading from
            // the checkpoint, so the state roots are different.
            let state_root = app_query.get_state_root().unwrap();
            assert_ne!(state_root, checkpoint_state_root);

            // NOTE: It's important that `app` and `app_query` are dropped here or else the app db
            // LOCK file will not be released, even when the node is released.
        }

        // Shutdown and drop the node.
        node.shutdown().await;
        drop(node);

        // Wait for the component database locks to be released.
        // This mitigates a race condition where the OS has not yet propagated the closing of the
        // database files, and so the next iteration will fail to open them.
        wait_for_database_locks(&config).await.unwrap();
    }
}

async fn wait_for_database_locks(config: &TomlConfigProvider<FullNodeComponents>) -> Result<()> {
    // Get the application db path before dropping the node.
    let app_config = config.get::<Application<FullNodeComponents>>();
    let app_db_path = app_config.db_path;

    // Get the consensus db path before dropping the node.
    let consensus_config = config.get::<Consensus<FullNodeComponents>>();
    let consensus_db_path = consensus_config.store_path;

    // Wait for the app db lock file to be released.
    if let Some(path) = app_db_path {
        wait_for_file_to_close(
            &path.join("LOCK"),
            Duration::from_secs(5),
            Duration::from_millis(100),
        )
        .await?;
    }

    // Wait for the narwhal db lock file to be released.
    wait_for_file_to_close(
        &consensus_db_path.join("0/LOCK"),
        Duration::from_secs(5),
        Duration::from_millis(100),
    )
    .await?;

    Ok(())
}

fn build_genesis(keystore_config: &KeystoreConfig) -> Result<Genesis> {
    let node_secret_key =
        NodeSecretKey::decode_pem(&read_to_string(&keystore_config.node_key_path)?)
            .context("failed to decode node secret key")?;
    let node_public_key = node_secret_key.to_pk();

    let consensus_secret_key =
        ConsensusSecretKey::decode_pem(&read_to_string(&keystore_config.consensus_key_path)?)
            .context("failed to decode consensus secret key")?;
    let consensus_public_key = consensus_secret_key.to_pk();

    Ok(Genesis {
        chain_id: 123,
        committee_size: 10,
        node_count: 100,
        min_stake: 1000,

        topology_target_k: 8,
        topology_min_nodes: 16,

        node_info: vec![GenesisNode {
            owner: AccountOwnerSecretKey::generate().to_pk().into(),
            primary_public_key: node_public_key,
            primary_domain: "127.0.0.1".parse().unwrap(),
            consensus_public_key,
            worker_domain: "127.0.0.1".parse().unwrap(),
            worker_public_key: node_public_key,
            ports: NodePorts {
                primary: 0,
                worker: 0,
                mempool: 0,
                rpc: 0,
                pool: 0,
                pinger: 0,
                handshake: HandshakePorts {
                    http: 0,
                    webrtc: 0,
                    webtransport: 0,
                },
            },
            stake: Default::default(),
            reputation: None,
            current_epoch_served: None,
            genesis_committee: true,
        }],

        ..Genesis::default()
    })
}

fn build_checkpoint(
    genesis: &Genesis,
    genesis_path: &ResolvedPathBuf,
) -> Result<([u8; 32], StateRootHash, Vec<u8>)> {
    let temp_dir = tempdir().unwrap();

    let mut env = Env::new(
        &ApplicationConfig {
            network: None,
            genesis_path: Some(genesis_path.clone()),
            storage: StorageConfig::RocksDb,
            db_path: Some(temp_dir.path().join("app").try_into().unwrap()),
            db_options: None,
            dev: None,
        },
        None,
    )
    .unwrap();

    // Apply genesis.
    env.apply_genesis_block(genesis.clone()).unwrap();

    // Get the state root.
    let state_root = env.query_runner().get_state_root().unwrap();

    // Build the checkpoint hash and checkpoint bytes.
    let (checkpoint_hash, checkpoint) = env.build_checkpoint().unwrap();

    Ok((checkpoint_hash, state_root, checkpoint))
}

fn build_node_config(
    temp_dir: &TempDir,
    genesis_path: ResolvedPathBuf,
    keystore_config: KeystoreConfig,
    ports: NodePorts,
) -> TomlConfigProvider<FullNodeComponents> {
    let path = temp_dir.path().to_path_buf();

    let config = TomlConfigProvider::<FullNodeComponents>::default();

    config.inject::<Application<FullNodeComponents>>(ApplicationConfig {
        network: None,
        genesis_path: Some(genesis_path),
        storage: StorageConfig::RocksDb,
        db_path: Some(path.join("data/app_db").try_into().unwrap()),
        db_options: None,
        dev: None,
    });

    config.inject::<Resolver<FullNodeComponents>>(ResolverConfig {
        store_path: path
            .join("data/resolver_store")
            .try_into()
            .expect("Failed to resolve path"),
    });
    config.inject::<Rpc<FullNodeComponents>>(RpcConfig {
        hmac_secret_dir: Some(temp_dir.path().to_path_buf()),
        ..RpcConfig::default_with_port(ports.rpc)
    });

    config.inject::<Consensus<FullNodeComponents>>(ConsensusConfig {
        store_path: path
            .join("data/narwhal_store")
            .try_into()
            .expect("Failed to resolve path"),
    });

    config.inject::<Keystore<FullNodeComponents>>(keystore_config);

    config.inject::<Blockstore<FullNodeComponents>>(BlockstoreConfig {
        root: path
            .join("data/blockstore")
            .try_into()
            .expect("Failed to resolve path"),
    });

    config.inject::<BlockstoreServer<FullNodeComponents>>(BlockstoreServerConfig::default());

    config.inject::<Handshake<FullNodeComponents>>(HandshakeConfig {
        // TODO: figure out how to have e2e testing for the different transports (browser oriented)
        transports: vec![TransportConfig::WebRTC(WebRtcConfig {
            address: ([0, 0, 0, 0], ports.handshake.webrtc).into(),
        })],
        http_address: ([127, 0, 0, 1], ports.handshake.http).into(),
        ..Default::default()
    });

    config.inject::<ServiceExecutor<FullNodeComponents>>(ServiceExecutorConfig {
        services: Default::default(),
        ..Default::default()
    });

    config.inject::<ReputationAggregator<FullNodeComponents>>(RepAggConfig {
        reporter_buffer_size: 1,
    });

    config.inject::<PoolProvider<FullNodeComponents>>(PoolConfig {
        address: format!("127.0.0.1:{}", ports.pool).parse().unwrap(),
        ..Default::default()
    });

    config.inject::<Syncronizer<FullNodeComponents>>(SyncronizerConfig {
        epoch_change_delta: Duration::from_secs(500),
    });

    config.inject::<Archive<FullNodeComponents>>(ArchiveConfig {
        is_archive: true,
        store_path: path
            .join("data/archive")
            .try_into()
            .expect("Failed to resolve path"),
    });

    config.inject::<Pinger<FullNodeComponents>>(PingerConfig {
        address: format!("127.0.0.1:{}", ports.pinger).parse().unwrap(),
        ping_interval: Duration::from_secs(5),
    });

    config.inject::<DeliveryAcknowledgmentAggregator<FullNodeComponents>>(
        DeliveryAcknowledgmentConfig {
            db_path: path
                .join("data/dack_aggregator")
                .try_into()
                .expect("Failed to resolve path"),
            ..Default::default()
        },
    );

    config.inject::<Checkpointer<FullNodeComponents>>(CheckpointerConfig {
        database: CheckpointerDatabaseConfig {
            path: path
                .join("data/checkpointer")
                .try_into()
                .expect("Failed to resolve path"),
        },
    });

    config.inject::<CommitteeBeaconComponent<FullNodeComponents>>(CommitteeBeaconConfig {
        database: CommitteeBeaconDatabaseConfig {
            path: path
                .join("data/committee_beacon")
                .try_into()
                .expect("Failed to resolve path"),
        },
        ..Default::default()
    });

    config
}
