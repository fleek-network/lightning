use std::time::Duration;

use fleek_crypto::{AccountOwnerSecretKey, SecretKey};
use lightning_application::app::Application;
use lightning_application::config::{Config as AppConfig, Mode, StorageConfig};
use lightning_application::genesis::{Genesis, GenesisNode};
use lightning_broadcast::Broadcast;
use lightning_interfaces::fdi::Provider;
use lightning_interfaces::infu_collection::{Collection, Node};
use lightning_interfaces::types::NodePorts;
use lightning_interfaces::{partial, KeystoreInterface};
use lightning_notifier::Notifier;
use lightning_pool::PoolProvider;
use lightning_rep_collector::ReputationAggregator;
use lightning_signer::Signer;
use lightning_test_utils::json_config::JsonConfigProvider;
use lightning_test_utils::keys::EphemeralKeystore;

use crate::config::Config;
use crate::resolver::Resolver;

partial!(TestBinding {
    ConfigProviderInterface = JsonConfigProvider;
    KeystoreInterface = EphemeralKeystore<Self>;
    ApplicationInterface = Application<Self>;
    SignerInterface = Signer<Self>;
    PoolInterface = PoolProvider<Self>;
    BroadcastInterface = Broadcast<Self>;
    ResolverInterface = Resolver<Self>;
    NotifierInterface = Notifier<Self>;
    ReputationAggregatorInterface = ReputationAggregator<Self>;
});

#[tokio::test]
async fn test_start_shutdown() {
    let keystore = EphemeralKeystore::<TestBinding>::default();
    let (consensus_secret_key, node_secret_key) =
        (keystore.get_bls_sk(), keystore.get_ed25519_sk());
    let node_public_key = node_secret_key.to_pk();
    let consensus_public_key = consensus_secret_key.to_pk();
    let owner_secret_key = AccountOwnerSecretKey::generate();
    let owner_public_key = owner_secret_key.to_pk();

    let mut genesis = Genesis::load().unwrap();

    genesis.node_info.push(GenesisNode::new(
        owner_public_key.into(),
        node_public_key,
        "127.0.0.1".parse().unwrap(),
        consensus_public_key,
        "127.0.0.1".parse().unwrap(),
        node_public_key,
        NodePorts {
            primary: 48000_u16,
            worker: 48101_u16,
            mempool: 48202_u16,
            rpc: 48300_u16,
            pool: 48400_u16,
            pinger: 48600_u16,
            handshake: Default::default(),
        },
        None,
        true,
    ));

    let path = std::env::temp_dir().join("resolver-test");
    if path.exists() {
        std::fs::remove_dir_all(&path).expect("Failed to clean up directory before test");
    }

    let mut node = Node::<TestBinding>::init_with_provider(
        Provider::default()
            .with(
                JsonConfigProvider::default()
                    .with::<Application<TestBinding>>(AppConfig {
                        genesis: Some(genesis),
                        mode: Mode::Test,
                        testnet: false,
                        storage: StorageConfig::InMemory,
                        db_path: None,
                        db_options: None,
                    })
                    .with::<Resolver<TestBinding>>(Config {
                        store_path: path.clone().try_into().unwrap(),
                    }),
            )
            .with(keystore),
    )
    .unwrap();

    // Now for the actual test
    node.start().await;
    tokio::time::sleep(Duration::from_secs(2)).await;
    node.shutdown().await;

    if path.exists() {
        std::fs::remove_dir_all(&path).expect("Failed to clean up directory after test");
    }
}
