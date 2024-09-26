use fleek_crypto::{AccountOwnerSecretKey, NodeSecretKey, SecretKey};
use hp_fixed::unsigned::HpUfixed;
use lightning_interfaces::types::{
    DeliveryAcknowledgmentProof,
    ExecutionError,
    TotalServed,
    UpdateMethod,
};
use lightning_interfaces::SyncQueryRunnerInterface;
use lightning_utils::application::QueryRunnerExt;
use tempfile::tempdir;

use super::utils::*;

#[tokio::test]
async fn test_pod_without_proof() {
    let temp_dir = tempdir().unwrap();

    let committee_size = 4;
    let (committee, keystore) = create_genesis_committee(committee_size);
    let (update_socket, query_runner) = test_init_app(&temp_dir, committee);

    let bandwidth_commodity = 1000;
    let compute_commodity = 2000;
    let bandwidth_pod =
        prepare_pod_request(bandwidth_commodity, 0, &keystore[0].node_secret_key, 1);
    let compute_pod = prepare_pod_request(compute_commodity, 1, &keystore[0].node_secret_key, 2);

    // run the delivery ack transaction
    run_updates(vec![bandwidth_pod, compute_pod], &update_socket).await;

    let node_idx = query_runner
        .pubkey_to_index(&keystore[0].node_secret_key.to_pk())
        .unwrap();
    assert_eq!(
        query_runner
            .get_current_epoch_served(&node_idx)
            .unwrap()
            .served,
        vec![bandwidth_commodity, compute_commodity]
    );

    let epoch = 0;

    assert_eq!(
        query_runner.get_total_served(&epoch).unwrap(),
        TotalServed {
            served: vec![bandwidth_commodity, compute_commodity],
            reward_pool: (0.1 * bandwidth_commodity as f64 + 0.2 * compute_commodity as f64).into()
        }
    );
}

#[tokio::test]
async fn test_submit_pod_reverts_account_key() {
    let temp_dir = tempdir().unwrap();

    let committee_size = 4;
    let (committee, _keystore) = create_genesis_committee(committee_size);
    let (update_socket, _query_runner) = test_init_app(&temp_dir, committee);

    // Account Secret Key
    let secret_key = AccountOwnerSecretKey::generate();
    let submit_pod = UpdateMethod::SubmitDeliveryAcknowledgmentAggregation {
        commodity: 2000,
        service_id: 1,
        proofs: vec![DeliveryAcknowledgmentProof],
        metadata: None,
    };
    let update = prepare_update_request_account(submit_pod, &secret_key, 1);
    expect_tx_revert(update, &update_socket, ExecutionError::OnlyNode).await;
}

#[tokio::test]
async fn test_submit_pod_reverts_node_does_not_exist() {
    let temp_dir = tempdir().unwrap();

    // Create a genesis committee and seed the application state with it.
    let committee_size = 4;
    let (committee, _keystore) = create_genesis_committee(committee_size);
    let (update_socket, _query_runner) = test_init_app(&temp_dir, committee);

    // Unknown Node Key (without Stake)
    let node_secret_key = NodeSecretKey::generate();
    let submit_pod = UpdateMethod::SubmitDeliveryAcknowledgmentAggregation {
        commodity: 2000,
        service_id: 1,
        proofs: vec![DeliveryAcknowledgmentProof],
        metadata: None,
    };
    let update = prepare_update_request_node(submit_pod, &node_secret_key, 1);
    expect_tx_revert(update, &update_socket, ExecutionError::NodeDoesNotExist).await;
}

#[tokio::test]
async fn test_submit_pod_reverts_insufficient_stake() {
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

    let submit_pod = UpdateMethod::SubmitDeliveryAcknowledgmentAggregation {
        commodity: 2000,
        service_id: 1,
        proofs: vec![DeliveryAcknowledgmentProof],
        metadata: None,
    };
    let update = prepare_update_request_node(submit_pod, &node_secret_key, 1);
    expect_tx_revert(update, &update_socket, ExecutionError::InsufficientStake).await;
}

#[tokio::test]
async fn test_submit_pod_reverts_invalid_service_id() {
    let temp_dir = tempdir().unwrap();

    let committee_size = 4;
    let (committee, keystore) = create_genesis_committee(committee_size);
    let (update_socket, _query_runner) = test_init_app(&temp_dir, committee);

    let update = prepare_pod_request(2000, 1069, &keystore[0].node_secret_key, 1);

    // run the delivery ack transaction
    expect_tx_revert(update, &update_socket, ExecutionError::InvalidServiceId).await;
}
