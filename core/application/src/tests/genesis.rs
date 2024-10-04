use hp_fixed::unsigned::HpUfixed;
use lightning_interfaces::types::{Metadata, Value};
use lightning_interfaces::{fdi, ApplicationInterface};
use lightning_node::Node;
use lightning_test_utils::json_config::JsonConfigProvider;
use tempfile::tempdir;

use super::utils::*;
use super::TestBinding;
use crate::config::StorageConfig;
use crate::{Application, ApplicationConfig};

#[tokio::test]
async fn test_genesis_configuration() {
    let temp_dir = tempdir().unwrap();

    // Init application + get the query and update socket
    let (_, query_runner) = init_app(&temp_dir, None);
    // Get the genesis parameters plus the initial committee
    let genesis = test_genesis();
    let genesis_committee = genesis.node_info;
    // For every member of the genesis committee they should have an initial stake of the min stake
    // Query to make sure that holds true
    for node in genesis_committee {
        let balance = get_staked(&query_runner, &node.primary_public_key);
        assert_eq!(HpUfixed::<18>::from(genesis.min_stake), balance);
    }
}

#[tokio::test]
async fn test_node_startup_without_genesis() {
    let config = ApplicationConfig {
        network: None,
        genesis_path: None,
        storage: StorageConfig::InMemory,
        db_path: None,
        db_options: None,
        dev: None,
    };
    let node = Node::<TestBinding>::init_with_provider(
        fdi::Provider::default()
            .with(JsonConfigProvider::default().with::<Application<TestBinding>>(config)),
    )
    .expect("failed to initialize node");

    let app = node.provider.get::<Application<TestBinding>>();
    let query = app.sync_query();

    // Check that no genesis has been applied yet.
    query.run(|ctx| {
        let metadata_table = ctx.get_table::<Metadata, Value>("metadata");
        assert!(metadata_table.get(Metadata::Epoch).is_none());
        assert!(metadata_table.get(Metadata::BlockNumber).is_none());
    });

    // Apply the genesis.
    assert!(app.apply_genesis(test_genesis()).await.unwrap());

    // Check that genesis has been applied.
    query.run(|ctx| {
        let metadata_table = ctx.get_table::<Metadata, Value>("metadata");
        assert_eq!(
            metadata_table.get(Metadata::Epoch).unwrap(),
            Value::Epoch(0)
        );
        assert_eq!(
            metadata_table.get(Metadata::BlockNumber).unwrap(),
            Value::BlockNumber(0)
        );
    });
}
