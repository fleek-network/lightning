use std::path::PathBuf;
use std::time::Duration;

use fleek_crypto::{
    AccountOwnerSecretKey,
    ClientPublicKey,
    ConsensusSecretKey,
    EthAddress,
    SecretKey,
};
use lightning_application::app::Application;
use lightning_application::config::{Config as AppConfig, Mode, StorageConfig};
use lightning_application::genesis::{Genesis, GenesisAccount};
use lightning_blockstore::blockstore::Blockstore;
use lightning_blockstore::config::Config as BlockstoreConfig;
use lightning_interfaces::{Collection, Node};
use lightning_interfaces::{fdi, partial};
use lightning_notifier::Notifier;
use lightning_signer::Signer;
use lightning_test_utils::json_config::JsonConfigProvider;
use lightning_test_utils::keys::EphemeralKeystore;
use serial_test::serial;

use crate::shim::{ServiceExecutor, ServiceExecutorConfig};

partial!(TestBinding {
    ConfigProviderInterface = JsonConfigProvider;
    ServiceExecutorInterface = ServiceExecutor<Self>;
    NotifierInterface = Notifier<Self>;
    KeystoreInterface = EphemeralKeystore<Self>;
    BlockstoreInterface = Blockstore<Self>;
    SignerInterface = Signer<Self>;
    ApplicationInterface = Application<Self>;
    //FetcherInterface = Fetcher<Self>;
    //OriginProviderInterface = OriginDemuxer<Self>;
    //BroadcastInterface = Broadcast<Self>;
    //BlockstoreServerInterface = BlockstoreServer<Self>;
    //ResolverInterface = Resolver<Self>;
    //PoolInterface = PoolProvider<Self>;
    //TopologyInterface = Topology<Self>;
    //ReputationAggregatorInterface = ReputationAggregator<Self>;
});

/// Initialize and start a node, with the service initialized but left unstarted,
/// so that the consumer of this function can implement services in the test.
async fn init_service_executor(
    genesis: Genesis,
    path: PathBuf,
    service_id: u32,
) -> Node<TestBinding> {
    let node = Node::<TestBinding>::init_with_provider(
        fdi::Provider::default().with(
            JsonConfigProvider::default()
                .with::<Blockstore<TestBinding>>(BlockstoreConfig {
                    root: path.join("dummy_blockstore").try_into().unwrap(),
                })
                .with::<Application<TestBinding>>(AppConfig {
                    genesis: Some(genesis),
                    mode: Mode::Test,
                    testnet: false,
                    storage: StorageConfig::InMemory,
                    db_path: None,
                    db_options: None,
                })
                .with::<ServiceExecutor<TestBinding>>(ServiceExecutorConfig {
                    services: [service_id].into_iter().collect(),
                    ipc_path: path.join("ipc").try_into().unwrap(),
                }),
        ),
    )
    .expect("failed to initialize node");

    node.start().await;

    // setup environment for [`fn_sdk::init_from_env`]
    std::env::set_var("BLOCKSTORE_PATH", path.join("dummy_blockstore"));
    std::env::set_var(
        "IPC_PATH",
        path.join("ipc").join(format!("service-{}", service_id)),
    );
    node
}

#[tokio::test]
#[serial]
async fn test_query_client_info() {
    let path = std::env::temp_dir().join("lightning-service-ex-test-1");
    if path.exists() {
        std::fs::remove_dir_all(&path).unwrap();
    }
    std::fs::create_dir_all(&path).unwrap();

    let secret_key = ConsensusSecretKey::generate();
    let client_pk = ClientPublicKey(secret_key.to_pk().0);
    let account_key = AccountOwnerSecretKey::generate();
    let address: EthAddress = account_key.to_pk().into();

    let mut genesis = Genesis::load().unwrap();
    genesis.client.insert(client_pk, address);
    genesis.account.push(GenesisAccount {
        public_key: address,
        flk_balance: 47_u32.into(),
        stables_balance: 0,
        bandwidth_balance: 27,
    });
    genesis.node_info.clear();

    let mut node = init_service_executor(genesis, path.clone(), 1069).await;
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Start the service
    fn_sdk::ipc::init_from_env();

    // Get the client bandwidth balance
    let balance = fn_sdk::api::query_client_bandwidth_balance(client_pk).await;
    assert_eq!(balance, 27);

    // Get the client FLK balance
    let balance = fn_sdk::api::query_client_flk_balance(client_pk).await;
    assert_eq!(balance, 47);

    if path.exists() {
        std::fs::remove_dir_all(&path).unwrap();
    }
    node.shutdown().await;
}

#[tokio::test]
#[serial]
async fn test_query_missing_client_info() {
    let path = std::env::temp_dir().join("lightning-service-ex-test-2");
    if path.exists() {
        std::fs::remove_dir_all(&path).unwrap();
    }
    std::fs::create_dir_all(&path).unwrap();

    let secret_key = ConsensusSecretKey::generate();
    let client_pk = ClientPublicKey(secret_key.to_pk().0);

    let mut genesis = Genesis::load().unwrap();
    genesis.node_info.clear();

    let mut node = init_service_executor(genesis, path.clone(), 1070).await;
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Start the service
    fn_sdk::ipc::init_from_env();

    // Get the client bandwidth balance
    let balance = fn_sdk::api::query_client_bandwidth_balance(client_pk).await;
    assert_eq!(balance, 0);

    // Get the client FLK balance
    let balance = fn_sdk::api::query_client_flk_balance(client_pk).await;
    assert_eq!(balance, 0);

    if path.exists() {
        std::fs::remove_dir_all(&path).unwrap();
    }

    node.shutdown().await
}
