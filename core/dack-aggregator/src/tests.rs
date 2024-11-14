use std::collections::HashSet;
use std::time::{Duration, SystemTime};

use fleek_crypto::{AccountOwnerSecretKey, SecretKey};
use lightning_application::app::Application;
use lightning_application::config::ApplicationConfig;
use lightning_interfaces::prelude::*;
use lightning_interfaces::types::{
    DeliveryAcknowledgment,
    DeliveryAcknowledgmentProof,
    Genesis,
    GenesisNode,
    GenesisPrices,
    GenesisService,
    NodePorts,
};
use lightning_node::Node;
use lightning_notifier::Notifier;
use lightning_signer::Signer;
use lightning_test_utils::consensus::{MockConsensus, MockConsensusConfig, MockForwarder};
use lightning_test_utils::json_config::JsonConfigProvider;
use lightning_test_utils::keys::EphemeralKeystore;
use lightning_utils::application::QueryRunnerExt;
use tempfile::{tempdir, TempDir};

use crate::{Config, DeliveryAcknowledgmentAggregator};

partial_node_components!(TestBinding {
    ConfigProviderInterface = JsonConfigProvider;
    KeystoreInterface = EphemeralKeystore<Self>;
    ApplicationInterface = Application<Self>;
    NotifierInterface = Notifier<Self>;
    SignerInterface = Signer<Self>;
    ForwarderInterface = MockForwarder<Self>;
    ConsensusInterface = MockConsensus<Self>;
    DeliveryAcknowledgmentAggregatorInterface = DeliveryAcknowledgmentAggregator<Self>;
});

async fn init_aggregator(temp_dir: &TempDir) -> Node<TestBinding> {
    let keystore = EphemeralKeystore::<TestBinding>::default();
    let (consensus_secret_key, node_secret_key) =
        (keystore.get_bls_sk(), keystore.get_ed25519_sk());
    let node_public_key = node_secret_key.to_pk();
    let consensus_public_key = consensus_secret_key.to_pk();
    let owner_secret_key = AccountOwnerSecretKey::generate();
    let owner_public_key = owner_secret_key.to_pk();

    let genesis = Genesis {
        epoch_start: SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64,
        epoch_time: 4000, // millis

        node_info: vec![GenesisNode::new(
            owner_public_key.into(),
            node_public_key,
            "127.0.0.1".parse().unwrap(),
            consensus_public_key,
            "127.0.0.1".parse().unwrap(),
            node_public_key,
            NodePorts {
                primary: 48000,
                worker: 48101,
                mempool: 48102,
                rpc: 48103,
                pool: 48104,
                pinger: 48106,
                handshake: Default::default(),
            },
            None,
            true,
        )],

        service: vec![GenesisService {
            id: 0,
            owner: owner_public_key.into(),
            commodity_type: types::CommodityTypes::Bandwidth,
        }],

        commodity_prices: vec![GenesisPrices {
            commodity: types::CommodityTypes::Bandwidth,
            price: 0.1,
        }],

        ..Default::default()
    };

    let genesis_path = genesis
        .write_to_dir(temp_dir.path().to_path_buf().try_into().unwrap())
        .unwrap();

    Node::<TestBinding>::init_with_provider(
        fdi::Provider::default()
            .with(
                JsonConfigProvider::default()
                    .with::<Application<TestBinding>>(ApplicationConfig::test(genesis_path))
                    .with::<MockConsensus<TestBinding>>(MockConsensusConfig {
                        min_ordering_time: 0,
                        max_ordering_time: 1,
                        probability_txn_lost: 0.0,
                        transactions_to_lose: HashSet::new(),
                        new_block_interval: Duration::from_secs(5),
                        block_buffering_interval: Duration::from_secs(0),
                        forwarder_transaction_to_error: HashSet::new(),
                    })
                    .with::<DeliveryAcknowledgmentAggregator<TestBinding>>(Config {
                        submit_interval: Duration::from_secs(1),
                        db_path: temp_dir.path().join("db").try_into().unwrap(),
                    }),
            )
            .with(keystore),
    )
    .unwrap()
}

#[tokio::test]
async fn test_shutdown_and_start_again() {
    let temp_dir = tempdir().unwrap();

    let mut node = init_aggregator(&temp_dir).await;

    node.start().await;
    tokio::time::sleep(Duration::from_secs(2)).await;
    node.shutdown().await;
}

#[tokio::test]
async fn test_submit_dack() {
    let temp_dir = tempdir().unwrap();

    let mut node = init_aggregator(&temp_dir).await;
    node.start().await;
    tokio::time::sleep(Duration::from_secs(1)).await;

    let query_runner = node
        .provider
        .get::<c!(TestBinding::ApplicationInterface::SyncExecutor)>();

    let socket = node
        .provider
        .get::<DeliveryAcknowledgmentAggregator<TestBinding>>()
        .socket();

    let service_id = 0;
    let commodity = 10;
    let dack = DeliveryAcknowledgment {
        service_id,
        commodity,
        proof: DeliveryAcknowledgmentProof,
        metadata: None,
    };
    socket.run(dack).await.unwrap();
    // Wait for aggregator to submit txn.
    tokio::time::sleep(Duration::from_secs(2)).await;

    let total_served = query_runner
        .get_total_served(&query_runner.get_current_epoch())
        .expect("there to be total served information");
    assert_eq!(total_served.served[service_id as usize], commodity);

    node.shutdown().await;
}
