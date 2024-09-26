use std::collections::BTreeMap;

use fleek_crypto::{AccountOwnerSecretKey, NodeSecretKey, SecretKey};
use hp_fixed::unsigned::HpUfixed;
use lightning_interfaces::types::{
    ContentUpdate,
    ExecutionData,
    ExecutionError,
    Participation,
    UpdateMethod,
};
use lightning_interfaces::SyncQueryRunnerInterface;
use lightning_utils::application::QueryRunnerExt;
use tempfile::tempdir;

use super::utils::*;

#[tokio::test]
async fn test_uptime_participation() {
    let temp_dir = tempdir().unwrap();

    let committee_size = 4;
    let (mut committee, keystore) = create_genesis_committee(committee_size);
    committee[0].reputation = Some(40);
    committee[1].reputation = Some(80);
    let (update_socket, query_runner) = test_init_app(&temp_dir, committee);

    let required_signals = calculate_required_signals(committee_size);

    let peer_1 = keystore[2].node_secret_key.to_pk();
    let peer_2 = keystore[3].node_secret_key.to_pk();
    let nonce = 1;

    // Add records in the content registry for all nodes.
    let updates = vec![ContentUpdate {
        uri: [0u8; 32],
        remove: false,
    }];
    let content_registry_update =
        prepare_content_registry_update(updates.clone(), &keystore[2].node_secret_key, 1);
    expect_tx_success(content_registry_update, &update_socket, ExecutionData::None).await;
    let content_registry_update =
        prepare_content_registry_update(updates, &keystore[3].node_secret_key, 1);
    expect_tx_success(content_registry_update, &update_socket, ExecutionData::None).await;

    // Assert that registries have been updated.
    let index_peer1 = query_runner.pubkey_to_index(&peer_1).unwrap();
    let content_registry1 = content_registry(&query_runner, &index_peer1);
    assert!(!content_registry1.is_empty());

    let index_peer2 = query_runner.pubkey_to_index(&peer_2).unwrap();
    let content_registry2 = content_registry(&query_runner, &index_peer2);
    assert!(!content_registry2.is_empty());

    let providers = uri_to_providers(&query_runner, &[0u8; 32]);
    assert_eq!(providers.len(), 2);

    let mut map = BTreeMap::new();
    let _ = update_reputation_measurements(
        &query_runner,
        &mut map,
        &peer_1,
        test_reputation_measurements(20),
    );
    let _ = update_reputation_measurements(
        &query_runner,
        &mut map,
        &peer_2,
        test_reputation_measurements(40),
    );

    submit_reputation_measurements(&update_socket, &keystore[0].node_secret_key, nonce, map).await;

    let mut map = BTreeMap::new();
    let _ = update_reputation_measurements(
        &query_runner,
        &mut map,
        &peer_1,
        test_reputation_measurements(30),
    );

    let _ = update_reputation_measurements(
        &query_runner,
        &mut map,
        &peer_2,
        test_reputation_measurements(45),
    );
    submit_reputation_measurements(&update_socket, &keystore[1].node_secret_key, nonce, map).await;

    let epoch = 0;
    // Change epoch so that rep scores will be calculated from the measurements.
    for node in keystore.iter().take(required_signals) {
        change_epoch(&update_socket, &node.node_secret_key, 2, epoch).await;
    }

    let node_info1 = get_node_info(&query_runner, &peer_1);
    let node_info2 = get_node_info(&query_runner, &peer_2);

    assert_eq!(node_info1.participation, Participation::False);
    assert_eq!(node_info2.participation, Participation::True);

    // Assert that registries have been updated.
    let content_registry1 = content_registry(&query_runner, &index_peer1);
    assert!(content_registry1.is_empty());

    let content_registry2 = content_registry(&query_runner, &index_peer2);
    assert!(!content_registry2.is_empty());

    let providers = uri_to_providers(&query_runner, &[0u8; 32]);
    assert_eq!(providers.len(), 1);
}

#[tokio::test]
async fn test_opt_in_reverts_account_key() {
    let temp_dir = tempdir().unwrap();

    // Create a genesis committee and seed the application state with it.
    let committee_size = 4;
    let (committee, _keystore) = create_genesis_committee(committee_size);
    let (update_socket, _query_runner) = test_init_app(&temp_dir, committee);

    // Account Secret Key
    let secret_key = AccountOwnerSecretKey::generate();
    let opt_in = UpdateMethod::OptIn {};
    let update = prepare_update_request_account(opt_in, &secret_key, 1);
    expect_tx_revert(update, &update_socket, ExecutionError::OnlyNode).await;
}

#[tokio::test]
async fn test_opt_in_reverts_node_does_not_exist() {
    let temp_dir = tempdir().unwrap();

    // Create a genesis committee and seed the application state with it.
    let committee_size = 4;
    let (committee, _keystore) = create_genesis_committee(committee_size);
    let (update_socket, _query_runner) = test_init_app(&temp_dir, committee);

    // Unknown Node Key (without Stake)
    let node_secret_key = NodeSecretKey::generate();
    let opt_in = UpdateMethod::OptIn {};
    let update = prepare_update_request_node(opt_in, &node_secret_key, 1);
    expect_tx_revert(update, &update_socket, ExecutionError::NodeDoesNotExist).await;
}

#[tokio::test]
async fn test_opt_in_reverts_insufficient_stake() {
    let temp_dir = tempdir().unwrap();

    // Create a genesis committee and seed the application state with it.
    let committee_size = 4;
    let (committee, _keystore) = create_genesis_committee(committee_size);
    let (update_socket, query_runner) = test_init_app(&temp_dir, committee);

    let owner_secret_key = AccountOwnerSecretKey::generate();
    // New Node key
    let node_secret_key = NodeSecretKey::generate();

    // Stake less than the minimum required amount.
    let minimum_stake_amount: HpUfixed<18> = query_runner.get_staking_amount().into();
    let less_than_minimum_stake_amount: HpUfixed<18> =
        minimum_stake_amount / HpUfixed::<18>::from(2u16);
    deposit_and_stake(
        &update_socket,
        &owner_secret_key,
        1,
        &less_than_minimum_stake_amount,
        &node_secret_key.to_pk(),
        [0; 96].into(),
    )
    .await;

    let opt_in = UpdateMethod::OptIn {};
    let update = prepare_update_request_node(opt_in, &node_secret_key, 1);
    expect_tx_revert(update, &update_socket, ExecutionError::InsufficientStake).await;
    assert_ne!(
        get_node_participation(&query_runner, &node_secret_key.to_pk()),
        Participation::OptedIn
    );
}

#[tokio::test]
async fn test_opt_in_works() {
    let temp_dir = tempdir().unwrap();

    // Create a genesis committee and seed the application state with it.
    let committee_size = 4;
    let (committee, _keystore) = create_genesis_committee(committee_size);
    let (update_socket, query_runner) = test_init_app(&temp_dir, committee);

    let owner_secret_key = AccountOwnerSecretKey::generate();
    // New Node
    let node_secret_key = NodeSecretKey::generate();
    let node_pub_key = node_secret_key.to_pk();

    // Stake less than the minimum required amount.
    let minimum_stake_amount: HpUfixed<18> = query_runner.get_staking_amount().into();
    deposit_and_stake(
        &update_socket,
        &owner_secret_key,
        1,
        &minimum_stake_amount,
        &node_pub_key,
        [0; 96].into(),
    )
    .await;

    assert_ne!(
        get_node_participation(&query_runner, &node_pub_key),
        Participation::OptedIn
    );

    let opt_in = UpdateMethod::OptIn {};
    let update = prepare_update_request_node(opt_in, &node_secret_key, 1);
    expect_tx_success(update, &update_socket, ExecutionData::None).await;

    assert_eq!(
        get_node_participation(&query_runner, &node_pub_key),
        Participation::OptedIn
    );
}

#[tokio::test]
async fn test_opt_out_reverts_account_key() {
    let temp_dir = tempdir().unwrap();

    // Create a genesis committee and seed the application state with it.
    let committee_size = 4;
    let (committee, _keystore) = create_genesis_committee(committee_size);
    let (update_socket, _query_runner) = test_init_app(&temp_dir, committee);

    // Account Secret Key
    let secret_key = AccountOwnerSecretKey::generate();
    let opt_out = UpdateMethod::OptOut {};
    let update = prepare_update_request_account(opt_out, &secret_key, 1);
    expect_tx_revert(update, &update_socket, ExecutionError::OnlyNode).await;
}

#[tokio::test]
async fn test_opt_out_reverts_node_does_not_exist() {
    let temp_dir = tempdir().unwrap();

    // Create a genesis committee and seed the application state with it.
    let committee_size = 4;
    let (committee, _keystore) = create_genesis_committee(committee_size);
    let (update_socket, _query_runner) = test_init_app(&temp_dir, committee);

    // Unknown Node Key (without Stake)
    let node_secret_key = NodeSecretKey::generate();
    let opt_out = UpdateMethod::OptOut {};
    let update = prepare_update_request_node(opt_out, &node_secret_key, 1);
    expect_tx_revert(update, &update_socket, ExecutionError::NodeDoesNotExist).await;
}

#[tokio::test]
async fn test_opt_out_reverts_insufficient_stake() {
    let temp_dir = tempdir().unwrap();

    // Create a genesis committee and seed the application state with it.
    let committee_size = 4;
    let (committee, _keystore) = create_genesis_committee(committee_size);
    let (update_socket, query_runner) = test_init_app(&temp_dir, committee);

    let owner_secret_key = AccountOwnerSecretKey::generate();
    // New Node key
    let node_secret_key = NodeSecretKey::generate();

    // Stake less than the minimum required amount.
    let minimum_stake_amount: HpUfixed<18> = query_runner.get_staking_amount().into();
    let less_than_minimum_stake_amount: HpUfixed<18> =
        minimum_stake_amount / HpUfixed::<18>::from(2u16);
    deposit_and_stake(
        &update_socket,
        &owner_secret_key,
        1,
        &less_than_minimum_stake_amount,
        &node_secret_key.to_pk(),
        [0; 96].into(),
    )
    .await;

    let opt_out = UpdateMethod::OptOut {};
    let update = prepare_update_request_node(opt_out, &node_secret_key, 1);
    expect_tx_revert(update, &update_socket, ExecutionError::InsufficientStake).await;
    assert_ne!(
        get_node_participation(&query_runner, &node_secret_key.to_pk()),
        Participation::OptedOut
    );
}

#[tokio::test]
async fn test_opt_out_works() {
    let temp_dir = tempdir().unwrap();

    // Create a genesis committee and seed the application state with it.
    let committee_size = 4;
    let (committee, _keystore) = create_genesis_committee(committee_size);
    let (update_socket, query_runner) = test_init_app(&temp_dir, committee);

    let owner_secret_key = AccountOwnerSecretKey::generate();
    // New Node
    let node_secret_key = NodeSecretKey::generate();
    let node_pub_key = node_secret_key.to_pk();

    // Stake less than the minimum required amount.
    let minimum_stake_amount: HpUfixed<18> = query_runner.get_staking_amount().into();
    deposit_and_stake(
        &update_socket,
        &owner_secret_key,
        1,
        &minimum_stake_amount,
        &node_pub_key,
        [0; 96].into(),
    )
    .await;

    assert_ne!(
        get_node_participation(&query_runner, &node_pub_key),
        Participation::OptedOut
    );

    let opt_out = UpdateMethod::OptOut {};
    let update = prepare_update_request_node(opt_out, &node_secret_key, 1);
    expect_tx_success(update, &update_socket, ExecutionData::None).await;

    assert_eq!(
        get_node_participation(&query_runner, &node_pub_key),
        Participation::OptedOut
    );
}
