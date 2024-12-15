use fleek_crypto::{AccountOwnerSecretKey, EthAddress, SecretKey};
use hp_fixed::unsigned::HpUfixed;
use lightning_interfaces::types::{
    ExecutionData,
    ExecutionError,
    GenesisAccount,
    ProofOfConsensus,
    Tokens,
    UpdateMethod,
};
use lightning_interfaces::SyncQueryRunnerInterface;
use tempfile::tempdir;

use super::utils::*;

#[tokio::test]
async fn test_revert_self_transfer() {
    let temp_dir = tempdir().unwrap();

    let (update_socket, query_runner) = init_app(&temp_dir, None);

    let owner_secret_key = AccountOwnerSecretKey::generate();
    let owner: EthAddress = owner_secret_key.to_pk().into();

    let balance = 1_000u64.into();

    deposit(&update_socket, &owner_secret_key, 1, &balance).await;
    assert_eq!(get_flk_balance(&query_runner, &owner), balance);

    // Check that trying to transfer funds to yourself reverts
    let update = prepare_transfer_request(&10_u64.into(), &owner, &owner_secret_key, 2);
    expect_tx_revert(update, &update_socket, ExecutionError::CantSendToYourself).await;

    // Assure that Flk balance has not changed
    assert_eq!(get_flk_balance(&query_runner, &owner), balance);
}

#[tokio::test]
async fn test_revert_transfer_not_account_key() {
    let temp_dir = tempdir().unwrap();

    let committee_size = 4;
    let (committee, keystore) = create_genesis_committee(committee_size);
    let (update_socket, query_runner) = test_init_app(&temp_dir, committee);
    let recipient: EthAddress = AccountOwnerSecretKey::generate().to_pk().into();

    let amount: HpUfixed<18> = 10_u64.into();
    let zero_balance = 0u64.into();

    assert_eq!(get_flk_balance(&query_runner, &recipient), zero_balance);

    let transfer = UpdateMethod::Transfer {
        amount: amount.clone(),
        token: Tokens::FLK,
        to: recipient,
    };

    // Check that trying to transfer funds with Node Key reverts
    let node_secret_key = &keystore[0].node_secret_key;
    let update_node_key = prepare_update_request_node(transfer.clone(), node_secret_key, 1);
    expect_tx_revert(
        update_node_key,
        &update_socket,
        ExecutionError::OnlyAccountOwner,
    )
    .await;

    // Check that trying to transfer funds with Consensus Key reverts
    let consensus_secret_key = &keystore[0].consensus_secret_key;
    let update_consensus_key = prepare_update_request_consensus(transfer, consensus_secret_key, 2);
    expect_tx_revert(
        update_consensus_key,
        &update_socket,
        ExecutionError::OnlyAccountOwner,
    )
    .await;

    // Assure that Flk balance has not changed
    assert_eq!(get_flk_balance(&query_runner, &recipient), zero_balance);
}

#[tokio::test]
async fn test_revert_transfer_when_insufficient_balance() {
    let temp_dir = tempdir().unwrap();

    let (update_socket, query_runner) = init_app(&temp_dir, None);

    let owner_secret_key = AccountOwnerSecretKey::generate();
    let recipient: EthAddress = AccountOwnerSecretKey::generate().to_pk().into();

    let balance = 10_u64.into();
    let zero_balance = 0u64.into();

    deposit(&update_socket, &owner_secret_key, 1, &balance).await;
    assert_eq!(get_flk_balance(&query_runner, &recipient), zero_balance);

    // Check that trying to transfer insufficient funds reverts
    let update = prepare_transfer_request(&11u64.into(), &recipient, &owner_secret_key, 2);
    expect_tx_revert(update, &update_socket, ExecutionError::InsufficientBalance).await;

    // Assure that Flk balance has not changed
    assert_eq!(get_flk_balance(&query_runner, &recipient), zero_balance);
}

#[tokio::test]
async fn test_transfer_works_properly() {
    let temp_dir = tempdir().unwrap();

    let (update_socket, query_runner) = init_app(&temp_dir, None);

    let owner_secret_key = AccountOwnerSecretKey::generate();
    let owner: EthAddress = owner_secret_key.to_pk().into();
    let recipient: EthAddress = AccountOwnerSecretKey::generate().to_pk().into();

    let balance = 1_000u64.into();
    let zero_balance = 0u64.into();
    let transfer_amount: HpUfixed<18> = 10_u64.into();

    deposit(&update_socket, &owner_secret_key, 1, &balance).await;

    assert_eq!(get_flk_balance(&query_runner, &owner), balance);
    assert_eq!(get_flk_balance(&query_runner, &recipient), zero_balance);

    // Check that trying to transfer funds to yourself reverts
    let update = prepare_transfer_request(&10_u64.into(), &recipient, &owner_secret_key, 2);
    expect_tx_success(update, &update_socket, ExecutionData::None).await;

    // Assure that Flk balance has decreased for sender
    assert_eq!(
        get_flk_balance(&query_runner, &owner),
        balance - transfer_amount.clone()
    );
    // Assure that Flk balance has increased for recipient
    assert_eq!(
        get_flk_balance(&query_runner, &recipient),
        zero_balance + transfer_amount
    );
}

#[tokio::test]
async fn test_deposit_flk_works_properly() {
    let temp_dir = tempdir().unwrap();

    let (update_socket, query_runner) = init_app(&temp_dir, None);

    let owner_secret_key = AccountOwnerSecretKey::generate();
    let owner: EthAddress = owner_secret_key.to_pk().into();

    let deposit_amount: HpUfixed<18> = 1_000u64.into();
    let intial_balance = get_flk_balance(&query_runner, &owner);

    let deposit = UpdateMethod::Deposit {
        proof: ProofOfConsensus {},
        token: Tokens::FLK,
        amount: deposit_amount.clone(),
    };
    let update = prepare_update_request_account(deposit, &owner_secret_key, 1);
    expect_tx_success(update, &update_socket, ExecutionData::None).await;

    assert_eq!(
        get_flk_balance(&query_runner, &owner),
        intial_balance + deposit_amount
    );
}

#[tokio::test]
async fn test_revert_deposit_not_account_key() {
    let temp_dir = tempdir().unwrap();

    let committee_size = 4;
    let (committee, keystore) = create_genesis_committee(committee_size);
    let (update_socket, _query_runner) = test_init_app(&temp_dir, committee);

    let amount: HpUfixed<18> = 10_u64.into();
    let deposit = UpdateMethod::Deposit {
        proof: ProofOfConsensus {},
        token: Tokens::FLK,
        amount,
    };

    // Check that trying to deposit funds with Node Key reverts
    let node_secret_key = &keystore[0].node_secret_key;
    let update_node_key = prepare_update_request_node(deposit.clone(), node_secret_key, 1);
    expect_tx_revert(
        update_node_key,
        &update_socket,
        ExecutionError::OnlyAccountOwner,
    )
    .await;

    // Check that trying to deposit funds with Consensus Key reverts
    let consensus_secret_key = &keystore[0].consensus_secret_key;
    let update_consensus_key = prepare_update_request_consensus(deposit, consensus_secret_key, 2);
    expect_tx_revert(
        update_consensus_key,
        &update_socket,
        ExecutionError::OnlyAccountOwner,
    )
    .await;
}

#[tokio::test]
async fn test_deposit_usdc_works_properly() {
    let temp_dir = tempdir().unwrap();

    let (update_socket, query_runner) = init_app(&temp_dir, None);

    let owner_secret_key = AccountOwnerSecretKey::generate();
    let owner: EthAddress = owner_secret_key.to_pk().into();

    let intial_balance = get_account_balance(&query_runner, &owner);
    let deposit_amount = 1_000;
    let deposit = UpdateMethod::Deposit {
        proof: ProofOfConsensus {},
        token: Tokens::USDC,
        amount: deposit_amount.into(),
    };
    let update = prepare_update_request_account(deposit, &owner_secret_key, 1);
    expect_tx_success(update, &update_socket, ExecutionData::None).await;

    assert_eq!(
        get_account_balance(&query_runner, &owner),
        intial_balance + deposit_amount
    );
}

#[tokio::test]
async fn test_withdraw_usdc_works_properly() {
    let temp_dir = tempdir().unwrap();

    let mut genesis = test_genesis();

    let owner_secret_key = AccountOwnerSecretKey::generate();
    let owner: EthAddress = owner_secret_key.to_pk().into();

    let receiver_secret_key = AccountOwnerSecretKey::generate();
    let receiver: EthAddress = receiver_secret_key.to_pk().into();

    let account = GenesisAccount {
        public_key: owner,
        flk_balance: 0_u64.into(),
        stables_balance: 1000,
        bandwidth_balance: 0,
    };
    genesis.account = vec![account];

    let (update_socket, query_runner) = init_app_with_genesis(&temp_dir, &genesis);

    let withdraw_amount = 500_u64;
    let withdraw = UpdateMethod::Withdraw {
        amount: withdraw_amount.into(),
        token: Tokens::USDC,
        receiving_address: receiver,
    };
    let update = prepare_update_request_account(withdraw, &owner_secret_key, 1);
    expect_tx_success(update, &update_socket, ExecutionData::None).await;

    let withdraws = query_runner.get_usdc_withdraws();
    assert_eq!(withdraws[0].1, receiver);
    assert_eq!(withdraws[0].2, withdraw_amount.into());
}

#[tokio::test]
async fn test_withdraw_flk_works_properly() {
    let temp_dir = tempdir().unwrap();

    let mut genesis = test_genesis();

    let owner_secret_key = AccountOwnerSecretKey::generate();
    let owner: EthAddress = owner_secret_key.to_pk().into();

    let receiver_secret_key = AccountOwnerSecretKey::generate();
    let receiver: EthAddress = receiver_secret_key.to_pk().into();

    let account = GenesisAccount {
        public_key: owner,
        flk_balance: 1000_u64.into(),
        stables_balance: 0,
        bandwidth_balance: 0,
    };
    genesis.account = vec![account];

    let (update_socket, query_runner) = init_app_with_genesis(&temp_dir, &genesis);

    let withdraw_amount = 500_u64;
    let withdraw = UpdateMethod::Withdraw {
        amount: withdraw_amount.into(),
        token: Tokens::FLK,
        receiving_address: receiver,
    };
    let update = prepare_update_request_account(withdraw, &owner_secret_key, 1);
    expect_tx_success(update, &update_socket, ExecutionData::None).await;

    let withdraws = query_runner.get_flk_withdraws();
    assert_eq!(withdraws[0].1, receiver);
    assert_eq!(withdraws[0].2, withdraw_amount.into());
}
