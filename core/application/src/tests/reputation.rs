use std::collections::BTreeMap;

use fleek_crypto::{AccountOwnerSecretKey, NodeSecretKey, SecretKey};
use hp_fixed::unsigned::HpUfixed;
use lightning_interfaces::types::{
    ExecutionData,
    ExecutionError,
    UpdateMethod,
    MAX_MEASUREMENTS_PER_TX,
    MAX_MEASUREMENTS_SUBMIT,
};
use lightning_interfaces::SyncQueryRunnerInterface;
use lightning_test_utils::e2e::TestNetwork;
use lightning_test_utils::random;
use lightning_test_utils::reputation::generate_reputation_measurements;
use lightning_utils::application::QueryRunnerExt;
use tempfile::tempdir;

use super::utils::*;

#[tokio::test]
async fn test_submit_rep_measurements() {
    let temp_dir = tempdir().unwrap();

    let committee_size = 4;
    let (committee, keystore) = create_genesis_committee(committee_size);
    let (update_socket, query_runner) = test_init_app(&temp_dir, committee);
    let mut rng = random::get_seedable_rng();

    let mut map = BTreeMap::new();
    let update1 = update_reputation_measurements(
        &query_runner,
        &mut map,
        &keystore[1].node_secret_key.to_pk(),
        generate_reputation_measurements(&mut rng, 0.1),
    );
    let update2 = update_reputation_measurements(
        &query_runner,
        &mut map,
        &keystore[2].node_secret_key.to_pk(),
        generate_reputation_measurements(&mut rng, 0.1),
    );

    let reporting_node_key = keystore[0].node_secret_key.to_pk();
    let reporting_node_index = get_node_index(&query_runner, &reporting_node_key);

    submit_reputation_measurements(&update_socket, &keystore[0].node_secret_key, 1, map).await;

    assert_rep_measurements_update!(&query_runner, update1, reporting_node_index);
    assert_rep_measurements_update!(&query_runner, update2, reporting_node_index);
}

#[tokio::test]
async fn test_submit_rep_measurements_too_many_times() {
    let temp_dir = tempdir().unwrap();

    let committee_size = 4;
    let (committee, keystore) = create_genesis_committee(committee_size);
    let (update_socket, query_runner) = test_init_app(&temp_dir, committee);

    let mut rng = random::get_seedable_rng();

    let mut map = BTreeMap::new();
    let _ = update_reputation_measurements(
        &query_runner,
        &mut map,
        &keystore[1].node_secret_key.to_pk(),
        generate_reputation_measurements(&mut rng, 0.1),
    );

    // Attempt to submit reputation measurements 1 more time than allowed per epoch.
    // This transaction should revert because each node only can submit its reputation measurements
    // `MAX_MEASUREMENTS_SUBMIT` times.
    for i in 0..MAX_MEASUREMENTS_SUBMIT {
        let req = prepare_update_request_node(
            UpdateMethod::SubmitReputationMeasurements {
                measurements: map.clone(),
            },
            &keystore[0].node_secret_key,
            1 + i as u64,
        );
        expect_tx_success(req, &update_socket, ExecutionData::None).await;
    }
    let req = prepare_update_request_node(
        UpdateMethod::SubmitReputationMeasurements { measurements: map },
        &keystore[0].node_secret_key,
        1 + MAX_MEASUREMENTS_SUBMIT as u64,
    );
    expect_tx_revert(
        req,
        &update_socket,
        ExecutionError::SubmittedTooManyTransactions,
    )
    .await;
}

#[tokio::test]
async fn test_rep_scores() {
    let mut network = TestNetwork::builder()
        .with_num_nodes(4)
        .with_genesis_mutator(|genesis| {
            genesis.node_info[0].reputation = Some(40);
            genesis.node_info[1].reputation = Some(80);
        })
        .build()
        .await
        .unwrap();
    let node = network.node(0);
    let peer1 = network.node(2);
    let peer2 = network.node(3);

    let mut rng = random::get_seedable_rng();

    // Submit reputation measurements from node 0, for peer 1 and 2.
    let measurements = BTreeMap::from_iter(vec![
        (
            peer1.index(),
            generate_reputation_measurements(&mut rng, 0.1),
        ),
        (
            peer2.index(),
            generate_reputation_measurements(&mut rng, 0.1),
        ),
    ]);
    node.execute_transaction_from_node(UpdateMethod::SubmitReputationMeasurements { measurements })
        .await
        .unwrap();

    // Submit reputation measurements from node 1, for peer 1 and 2.
    let measurements = BTreeMap::from_iter(vec![
        (
            peer1.index(),
            generate_reputation_measurements(&mut rng, 0.1),
        ),
        (
            peer2.index(),
            generate_reputation_measurements(&mut rng, 0.1),
        ),
    ]);
    node.execute_transaction_from_node(UpdateMethod::SubmitReputationMeasurements { measurements })
        .await
        .unwrap();

    // Change epoch and wait for it to be complete.
    network.change_epoch_and_wait_for_complete().await.unwrap();

    // Check the reputation scores.
    assert!(
        node.app_query
            .get_reputation_score(&peer1.index())
            .is_some()
    );
    assert!(
        node.app_query
            .get_reputation_score(&peer2.index())
            .is_some()
    );

    // Shutdown the network.
    network.shutdown().await;
}

#[tokio::test]
async fn test_submit_reputation_measurements_reverts_account_key() {
    let temp_dir = tempdir().unwrap();

    // Create a genesis committee and seed the application state with it.
    let committee_size = 4;
    let (committee, _keystore) = create_genesis_committee(committee_size);
    let (update_socket, _query_runner) = test_init_app(&temp_dir, committee);

    // Account Secret Key
    let secret_key = AccountOwnerSecretKey::generate();
    let opt_in = UpdateMethod::SubmitReputationMeasurements {
        measurements: Default::default(),
    };
    let update = prepare_update_request_account(opt_in, &secret_key, 1);
    expect_tx_revert(update, &update_socket, ExecutionError::OnlyNode).await;
}

#[tokio::test]
async fn test_submit_reputation_measurements_reverts_node_does_not_exist() {
    let temp_dir = tempdir().unwrap();

    let committee_size = 4;
    let (committee, keystore) = create_genesis_committee(committee_size);
    let (update_socket, query_runner) = test_init_app(&temp_dir, committee);
    let mut rng = random::get_seedable_rng();

    let mut measurements = BTreeMap::new();
    let _ = update_reputation_measurements(
        &query_runner,
        &mut measurements,
        &keystore[1].node_secret_key.to_pk(),
        generate_reputation_measurements(&mut rng, 0.1),
    );

    let update = prepare_update_request_node(
        UpdateMethod::SubmitReputationMeasurements { measurements },
        &NodeSecretKey::generate(),
        1,
    );

    expect_tx_revert(update, &update_socket, ExecutionError::NodeDoesNotExist).await;
}

#[tokio::test]
async fn test_submit_reputation_measurements_reverts_insufficient_stake() {
    let temp_dir = tempdir().unwrap();

    let committee_size = 4;
    let (committee, keystore) = create_genesis_committee(committee_size);
    let (update_socket, query_runner) = test_init_app(&temp_dir, committee);
    let mut rng = random::get_seedable_rng();

    let owner_secret_key = AccountOwnerSecretKey::generate();
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

    let mut measurements = BTreeMap::new();
    let _ = update_reputation_measurements(
        &query_runner,
        &mut measurements,
        &keystore[1].node_secret_key.to_pk(),
        generate_reputation_measurements(&mut rng, 0.1),
    );

    let update = prepare_update_request_node(
        UpdateMethod::SubmitReputationMeasurements { measurements },
        &node_secret_key,
        1,
    );

    expect_tx_revert(update, &update_socket, ExecutionError::InsufficientStake).await;
}

#[tokio::test]
async fn test_submit_reputation_measurements_too_many_measurements() {
    let temp_dir = tempdir().unwrap();

    let committee_size = 4;
    let (committee, _keystore) = create_genesis_committee(committee_size);
    let (update_socket, query_runner) = test_init_app(&temp_dir, committee);
    let mut rng = random::get_seedable_rng();

    let owner_secret_key = AccountOwnerSecretKey::generate();
    let node_secret_key = NodeSecretKey::generate();

    // Stake minimum required amount.
    deposit_and_stake(
        &update_socket,
        &owner_secret_key,
        1,
        &query_runner.get_staking_amount().into(),
        &node_secret_key.to_pk(),
        [0; 96].into(),
    )
    .await;

    let mut measurements = BTreeMap::new();

    // create many dummy measurements that len >
    for i in 1..MAX_MEASUREMENTS_PER_TX + 2 {
        measurements.insert(i as u32, generate_reputation_measurements(&mut rng, 0.5));
    }
    let update = prepare_update_request_node(
        UpdateMethod::SubmitReputationMeasurements { measurements },
        &node_secret_key,
        1,
    );

    expect_tx_revert(update, &update_socket, ExecutionError::TooManyMeasurements).await;
}
