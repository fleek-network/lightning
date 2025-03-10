use std::collections::BTreeMap;
use std::time::Duration;

use fleek_crypto::{AccountOwnerSecretKey, NodeSecretKey, SecretKey};
use hp_fixed::unsigned::HpUfixed;
use lightning_interfaces::types::{
    CommitteeSelectionBeaconCommit,
    ExecutionData,
    ExecutionError,
    Participation,
    UpdateMethod,
};
use lightning_interfaces::SyncQueryRunnerInterface;
use lightning_test_utils::e2e::{TestFullNodeComponentsWithMockConsensus, TestNetwork};
use lightning_utils::application::QueryRunnerExt;
use lightning_utils::poll::{poll_until, PollUntilError};
use tempfile::tempdir;

use super::utils::*;

#[tokio::test]
async fn test_uptime_participation() {
    let commit_phase_duration = 2000;
    let reveal_phase_duration = 2000;
    let mut network = TestNetwork::builder()
        .with_committee_nodes::<TestFullNodeComponentsWithMockConsensus>(4)
        .await
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

    // Add records in the content registry for the peers.
    peer1
        .execute_transaction_from_node(UpdateMethod::UpdateContentRegistry {
            updates: vec![Default::default()],
        })
        .await
        .unwrap();
    peer2
        .execute_transaction_from_node(UpdateMethod::UpdateContentRegistry {
            updates: vec![Default::default()],
        })
        .await
        .unwrap();

    // Check that the content registries have been updated.
    poll_until(
        || async {
            let providers = node
                .app_query()
                .get_uri_providers(&[0u8; 32])
                .unwrap_or_default();
            (providers.len() == 2)
                .then_some(())
                .ok_or(PollUntilError::ConditionNotSatisfied)
        },
        Duration::from_secs(3),
        Duration::from_millis(100),
    )
    .await
    .unwrap();
    assert!(!node
        .app_query()
        .get_content_registry(&peer1.index())
        .unwrap_or_default()
        .is_empty());
    assert!(!node
        .app_query()
        .get_content_registry(&peer2.index())
        .unwrap_or_default()
        .is_empty());

    // Submit reputation measurements from node 0, for peer 1 and 2.
    let measurements: BTreeMap<u32, lightning_interfaces::types::ReputationMeasurements> =
        BTreeMap::from_iter(vec![
            (peer1.index(), test_reputation_measurements(20)),
            (peer2.index(), test_reputation_measurements(40)),
        ]);
    network
        .node(0)
        .execute_transaction_from_node(UpdateMethod::SubmitReputationMeasurements { measurements })
        .await
        .unwrap();

    // Submit reputation measurements from node 1, for peer 1 and 2.
    let measurements = BTreeMap::from_iter(vec![
        (peer1.index(), test_reputation_measurements(30)),
        (peer2.index(), test_reputation_measurements(45)),
    ]);
    network
        .node(1)
        .execute_transaction_from_node(UpdateMethod::SubmitReputationMeasurements { measurements })
        .await
        .unwrap();

    // Change epoch and wait for it to be complete.
    network
        .change_epoch_and_wait_for_complete(0, commit_phase_duration, reveal_phase_duration)
        .await
        .unwrap();

    // Check participation.
    assert_eq!(
        peer1
            .app_query()
            .get_node_info(&peer1.index(), |n| n.participation)
            .unwrap(),
        Participation::False
    );
    assert_eq!(
        peer2
            .app_query()
            .get_node_info(&peer2.index(), |n| n.participation)
            .unwrap(),
        Participation::True
    );

    // Check that the content registries have been updated.
    let peer1_content_registry = node
        .app_query()
        .get_content_registry(&peer1.index())
        .unwrap_or_default();
    assert!(peer1_content_registry.is_empty());

    let peer2_content_registry = node
        .app_query()
        .get_content_registry(&peer2.index())
        .unwrap_or_default();
    assert!(!peer2_content_registry.is_empty());

    let providers = node
        .app_query()
        .get_uri_providers(&[0u8; 32])
        .unwrap_or_default();
    assert_eq!(providers.len(), 1);

    // Shutdown the network.
    network.shutdown().await;
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
async fn test_opt_out_success() {
    let temp_dir = tempdir().unwrap();

    // Create a genesis committee and seed the application state with it.
    let committee_size = 4;
    let (committee, keystore) = create_genesis_committee(committee_size);
    let (update_socket, query_runner) = test_init_app(&temp_dir, committee);

    let owner_secret_key = AccountOwnerSecretKey::generate();
    let node_secret_key = NodeSecretKey::generate();
    let node_pub_key = node_secret_key.to_pk();

    // Stake the minimum required amount.
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

    // Check that the node is initially not participating.
    assert_eq!(
        get_node_participation(&query_runner, &node_pub_key),
        Participation::False
    );

    // Execute opt-in transaction.
    expect_tx_success(
        prepare_update_request_node(UpdateMethod::OptIn {}, &node_secret_key, 1),
        &update_socket,
        ExecutionData::None,
    )
    .await;

    // Check that the node is now participating as opted-in.
    assert_eq!(
        get_node_participation(&query_runner, &node_pub_key),
        Participation::OptedIn
    );

    // Change epoch.
    expect_tx_success(
        prepare_update_request_node(
            UpdateMethod::ChangeEpoch { epoch: 0 },
            &keystore[0].node_secret_key,
            1,
        ),
        &update_socket,
        ExecutionData::None,
    )
    .await;
    expect_tx_success(
        prepare_update_request_node(
            UpdateMethod::ChangeEpoch { epoch: 0 },
            &keystore[1].node_secret_key,
            1,
        ),
        &update_socket,
        ExecutionData::None,
    )
    .await;
    expect_tx_success(
        prepare_update_request_node(
            UpdateMethod::ChangeEpoch { epoch: 0 },
            &keystore[2].node_secret_key,
            1,
        ),
        &update_socket,
        ExecutionData::None,
    )
    .await;

    expect_tx_success(
        prepare_update_request_node(
            UpdateMethod::CommitteeSelectionBeaconCommit {
                commit: CommitteeSelectionBeaconCommit::build(0, 0, [1; 32]),
            },
            &keystore[0].node_secret_key,
            2,
        ),
        &update_socket,
        ExecutionData::None,
    )
    .await;
    expect_tx_success(
        prepare_update_request_node(
            UpdateMethod::CommitteeSelectionBeaconCommit {
                commit: CommitteeSelectionBeaconCommit::build(0, 0, [2; 32]),
            },
            &keystore[1].node_secret_key,
            2,
        ),
        &update_socket,
        ExecutionData::None,
    )
    .await;
    expect_tx_success(
        prepare_update_request_node(
            UpdateMethod::CommitteeSelectionBeaconCommit {
                commit: CommitteeSelectionBeaconCommit::build(0, 0, [3; 32]),
            },
            &keystore[2].node_secret_key,
            2,
        ),
        &update_socket,
        ExecutionData::None,
    )
    .await;

    expect_tx_success(
        prepare_update_request_node(
            UpdateMethod::CommitteeSelectionBeaconCommitPhaseTimeout { epoch: 0, round: 0 },
            &keystore[0].node_secret_key,
            3,
        ),
        &update_socket,
        ExecutionData::None,
    )
    .await;
    expect_tx_success(
        prepare_update_request_node(
            UpdateMethod::CommitteeSelectionBeaconCommitPhaseTimeout { epoch: 0, round: 0 },
            &keystore[1].node_secret_key,
            3,
        ),
        &update_socket,
        ExecutionData::None,
    )
    .await;
    expect_tx_success(
        prepare_update_request_node(
            UpdateMethod::CommitteeSelectionBeaconCommitPhaseTimeout { epoch: 0, round: 0 },
            &keystore[2].node_secret_key,
            3,
        ),
        &update_socket,
        ExecutionData::None,
    )
    .await;

    expect_tx_success(
        prepare_update_request_node(
            UpdateMethod::CommitteeSelectionBeaconReveal { reveal: [1; 32] },
            &keystore[0].node_secret_key,
            4,
        ),
        &update_socket,
        ExecutionData::None,
    )
    .await;
    expect_tx_success(
        prepare_update_request_node(
            UpdateMethod::CommitteeSelectionBeaconReveal { reveal: [2; 32] },
            &keystore[1].node_secret_key,
            4,
        ),
        &update_socket,
        ExecutionData::None,
    )
    .await;
    expect_tx_success(
        prepare_update_request_node(
            UpdateMethod::CommitteeSelectionBeaconReveal { reveal: [3; 32] },
            &keystore[2].node_secret_key,
            4,
        ),
        &update_socket,
        ExecutionData::None,
    )
    .await;

    expect_tx_success(
        prepare_update_request_node(
            UpdateMethod::CommitteeSelectionBeaconRevealPhaseTimeout { epoch: 0, round: 0 },
            &keystore[0].node_secret_key,
            5,
        ),
        &update_socket,
        ExecutionData::None,
    )
    .await;
    expect_tx_success(
        prepare_update_request_node(
            UpdateMethod::CommitteeSelectionBeaconRevealPhaseTimeout { epoch: 0, round: 0 },
            &keystore[1].node_secret_key,
            5,
        ),
        &update_socket,
        ExecutionData::None,
    )
    .await;
    expect_tx_success(
        prepare_update_request_node(
            UpdateMethod::CommitteeSelectionBeaconRevealPhaseTimeout { epoch: 0, round: 0 },
            &keystore[2].node_secret_key,
            5,
        ),
        &update_socket,
        ExecutionData::EpochChange,
    )
    .await;

    // Execute opt-out transaction.
    let opt_out = UpdateMethod::OptOut {};
    let update = prepare_update_request_node(opt_out, &node_secret_key, 2);
    expect_tx_success(update, &update_socket, ExecutionData::None).await;

    // Check that the node is now not participating as opted-out.
    assert_eq!(
        get_node_participation(&query_runner, &node_pub_key),
        Participation::OptedOut
    );
}

#[tokio::test]
async fn test_opt_out_reverts_when_participation_is_false() {
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

    // Check that the node is initially not participating.
    assert_eq!(
        get_node_participation(&query_runner, &node_pub_key),
        Participation::False
    );

    // Execute opt-out transaction.
    let opt_out = UpdateMethod::OptOut {};
    let update = prepare_update_request_node(opt_out, &node_secret_key, 1);
    expect_tx_revert(update, &update_socket, ExecutionError::NodeNotParticipating).await;

    // Check that the node is still not participating.
    assert_eq!(
        get_node_participation(&query_runner, &node_pub_key),
        Participation::False
    );
}
