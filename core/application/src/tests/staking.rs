use std::net::IpAddr;
use std::time::Duration;

use fleek_crypto::{
    AccountOwnerSecretKey,
    ConsensusPublicKey,
    EthAddress,
    NodePublicKey,
    NodeSecretKey,
    SecretKey,
};
use hp_fixed::unsigned::HpUfixed;
use lightning_committee_beacon::CommitteeBeaconConfig;
use lightning_interfaces::types::{
    CommitteeSelectionBeaconCommit,
    ExecutionData,
    ExecutionError,
    HandshakePorts,
    NodePorts,
    NodeRegistryChange,
    Participation,
    UpdateMethod,
};
use lightning_interfaces::{KeystoreInterface, SyncQueryRunnerInterface};
use lightning_test_utils::consensus::MockConsensusConfig;
use lightning_test_utils::e2e::{
    DowncastToTestFullNode,
    TestFullNodeComponentsWithMockConsensus,
    TestNetwork,
    TestNetworkNode,
};
use lightning_utils::application::QueryRunnerExt;
use tempfile::tempdir;
use utils::{
    create_genesis_committee,
    deposit,
    deposit_and_stake,
    expect_tx_revert,
    expect_tx_success,
    get_flk_balance,
    get_locked,
    get_locked_time,
    get_node_info,
    get_stake_locked_until,
    get_staked,
    init_app,
    prepare_deposit_update,
    prepare_initial_stake_update,
    prepare_regular_stake_update,
    prepare_stake_lock_update,
    prepare_unstake_update,
    prepare_update_request_consensus,
    prepare_update_request_node,
    prepare_withdraw_unstaked_update,
    run_update,
    run_updates,
    test_genesis,
    test_init_app,
};

use super::*;

#[tokio::test]
async fn test_stake() {
    let temp_dir = tempdir().unwrap();

    let committee_size = 4;
    let (committee, _keystore) = create_genesis_committee(committee_size);
    let (update_socket, query_runner) = test_init_app(&temp_dir, committee);

    // Check that node registry changes were recorded for genesis nodes.
    let node_registry_changes = query_runner
        .get_committee_info(&0, |c| c.node_registry_changes)
        .unwrap();
    assert_eq!(node_registry_changes.len(), 1);
    assert_eq!(node_registry_changes.get(&0).unwrap().len(), committee_size);
    assert!(node_registry_changes
        .get(&0)
        .unwrap()
        .iter()
        .all(|(_, change)| { matches!(change, NodeRegistryChange::New) }));

    let owner_secret_key = AccountOwnerSecretKey::generate();
    let peer_pub_key = NodeSecretKey::generate().to_pk();

    // Deposit some FLK into account 1
    let deposit = 1000_u64.into();
    let update1 = prepare_deposit_update(&deposit, &owner_secret_key, 1);
    let update2 = prepare_deposit_update(&deposit, &owner_secret_key, 2);

    // Put 2 of the transaction in the block just to also test block exucution a bit
    run_updates(vec![update1, update2], &update_socket).await;

    // check that he has 2_000 flk balance
    assert_eq!(
        get_flk_balance(&query_runner, &owner_secret_key.to_pk().into()),
        (HpUfixed::<18>::from(2u16) * deposit)
    );

    // Test staking on a new node
    let stake_amount = 1000u64.into();
    // First check that trying to stake without providing all the node info reverts
    let update = prepare_regular_stake_update(&stake_amount, &peer_pub_key, &owner_secret_key, 3);
    expect_tx_revert(
        update,
        &update_socket,
        ExecutionError::InsufficientNodeDetails,
    )
    .await;

    // Now try with the correct details for a new node
    let update = prepare_initial_stake_update(
        &stake_amount,
        &peer_pub_key,
        [0; 96].into(),
        "127.0.0.1".parse().unwrap(),
        [0; 32].into(),
        "127.0.0.1".parse().unwrap(),
        NodePorts::default(),
        &owner_secret_key,
        4,
    );

    let resp = expect_tx_success(update, &update_socket, ExecutionData::None).await;

    // Check that node registry changes were recorded for the new node.
    let node_registry_changes = query_runner
        .get_committee_info(&0, |c| c.node_registry_changes)
        .unwrap();
    assert_eq!(node_registry_changes.len(), 2);
    assert_eq!(node_registry_changes.get(&0).unwrap().len(), committee_size);
    let expected_changes = vec![(peer_pub_key, NodeRegistryChange::New)];
    assert_eq!(node_registry_changes.get(&3).unwrap(), &expected_changes);
    assert_eq!(resp.node_registry_changes, expected_changes);

    // Query the new node and make sure he has the proper stake
    assert_eq!(get_staked(&query_runner, &peer_pub_key), stake_amount);

    // Stake 1000 more but since it is not a new node we should be able to leave the optional
    // parameters out without a revert
    let update = prepare_regular_stake_update(&stake_amount, &peer_pub_key, &owner_secret_key, 5);

    let resp = expect_tx_success(update, &update_socket, ExecutionData::None).await;

    // Check that no new node registry changes were recorded.
    let node_registry_changes = query_runner
        .get_committee_info(&0, |c| c.node_registry_changes)
        .unwrap();
    assert_eq!(node_registry_changes.len(), 2);
    assert_eq!(node_registry_changes.get(&0).unwrap().len(), committee_size);
    assert_eq!(node_registry_changes.get(&3).unwrap().len(), 1);
    assert!(resp.node_registry_changes.is_empty());

    // Node should now have 2_000 stake
    assert_eq!(
        get_staked(&query_runner, &peer_pub_key),
        (HpUfixed::<18>::from(2u16) * stake_amount.clone())
    );

    // Now test unstake and make sure it moves the tokens to locked status
    let update = prepare_unstake_update(&stake_amount, &peer_pub_key, &owner_secret_key, 6);
    run_update(update, &update_socket).await;

    // Check that his locked is 1000 and his remaining stake is 1000
    assert_eq!(get_staked(&query_runner, &peer_pub_key), stake_amount);
    assert_eq!(get_locked(&query_runner, &peer_pub_key), stake_amount);

    // Since this test starts at epoch 0 locked_until will be == lock_time
    assert_eq!(
        get_locked_time(&query_runner, &peer_pub_key),
        test_genesis().lock_time
    );

    // Try to withdraw the locked tokens and it should revert
    let update = prepare_withdraw_unstaked_update(&peer_pub_key, None, &owner_secret_key, 7);

    expect_tx_revert(update, &update_socket, ExecutionError::TokensLocked).await;
}

#[tokio::test]
async fn test_stake_lock() {
    let temp_dir = tempdir().unwrap();

    let (update_socket, query_runner) = init_app(&temp_dir, None);

    let owner_secret_key = AccountOwnerSecretKey::generate();
    let node_pub_key = NodeSecretKey::generate().to_pk();
    let amount: HpUfixed<18> = 1_000u64.into();

    deposit_and_stake(
        &update_socket,
        &owner_secret_key,
        1,
        &amount,
        &node_pub_key,
        [0; 96].into(),
    )
    .await;

    assert_eq!(get_staked(&query_runner, &node_pub_key), amount);

    let locked_for = 365;
    let stake_lock_req = prepare_stake_lock_update(&node_pub_key, locked_for, &owner_secret_key, 3);

    expect_tx_success(stake_lock_req, &update_socket, ExecutionData::None).await;

    assert_eq!(
        get_stake_locked_until(&query_runner, &node_pub_key),
        locked_for
    );

    let unstake_req = prepare_unstake_update(&amount, &node_pub_key, &owner_secret_key, 4);
    expect_tx_revert(
        unstake_req,
        &update_socket,
        ExecutionError::LockedTokensUnstakeForbidden,
    )
    .await;
}

#[tokio::test]
async fn test_stake_works() {
    let temp_dir = tempdir().unwrap();

    let committee_size = 4;
    let (committee, _keystore) = create_genesis_committee(committee_size);
    let (update_socket, query_runner) = test_init_app(&temp_dir, committee);

    let owner_secret_key = AccountOwnerSecretKey::generate();
    let address: EthAddress = owner_secret_key.to_pk().into();

    let peer_pub_key = NodeSecretKey::generate().to_pk();

    // Deposit some FLK into an account
    let stake = 1000_u64.into();
    deposit(&update_socket, &owner_secret_key, 1, &stake).await;

    let balance = get_flk_balance(&query_runner, &address);
    let consensus_key: ConsensusPublicKey = [0; 96].into();
    let node_domain: IpAddr = "89.64.54.26".parse().unwrap();
    let worker_pub_key: NodePublicKey = [0; 32].into();
    let worker_domain: IpAddr = "127.0.0.1".parse().unwrap();
    let node_ports = NodePorts {
        primary: 4001,
        worker: 4002,
        mempool: 4003,
        rpc: 4004,
        pool: 4005,
        pinger: 4007,
        handshake: HandshakePorts {
            http: 5001,
            webrtc: 5002,
            webtransport: 5003,
        },
    };
    // Now try with the correct details for a new node
    let update = prepare_initial_stake_update(
        &stake,
        &peer_pub_key,
        consensus_key,
        node_domain,
        worker_pub_key,
        worker_domain,
        node_ports.clone(),
        &owner_secret_key,
        2,
    );

    // Expect Success
    expect_tx_success(update, &update_socket, ExecutionData::None).await;

    // Flk balance has not changed
    assert_eq!(
        get_flk_balance(&query_runner, &address),
        balance - stake.clone()
    );

    let node_info = get_node_info(&query_runner, &peer_pub_key);
    assert_eq!(node_info.consensus_key, consensus_key);
    assert_eq!(node_info.domain, node_domain);
    assert_eq!(node_info.worker_public_key, worker_pub_key);
    assert_eq!(node_info.worker_domain, worker_domain);
    assert_eq!(node_info.ports, node_ports);

    // Query the new node and make sure he has the proper stake
    assert_eq!(get_staked(&query_runner, &peer_pub_key), stake);

    let node_idx = query_runner.pubkey_to_index(&peer_pub_key).unwrap();
    assert_eq!(
        query_runner.index_to_pubkey(&node_idx).unwrap(),
        peer_pub_key
    );
}

#[tokio::test]
async fn test_stake_lock_reverts_not_account_key() {
    let temp_dir = tempdir().unwrap();

    let committee_size = 4;
    let (committee, keystore) = create_genesis_committee(committee_size);
    let (update_socket, _query_runner) = test_init_app(&temp_dir, committee);

    let stake_lock = UpdateMethod::StakeLock {
        node: NodeSecretKey::generate().to_pk(),
        locked_for: 365,
    };

    // Check that trying to StakeLock funds with Node Key reverts
    let node_secret_key = &keystore[0].node_secret_key;
    let update_node_key = prepare_update_request_node(stake_lock.clone(), node_secret_key, 1);
    expect_tx_revert(
        update_node_key,
        &update_socket,
        ExecutionError::OnlyAccountOwner,
    )
    .await;

    // Check that trying to StakeLock funds with Consensus Key reverts
    let consensus_secret_key = &keystore[0].consensus_secret_key;
    let update_consensus_key =
        prepare_update_request_consensus(stake_lock, consensus_secret_key, 2);
    expect_tx_revert(
        update_consensus_key,
        &update_socket,
        ExecutionError::OnlyAccountOwner,
    )
    .await;
}

#[tokio::test]
async fn test_stake_lock_reverts_node_does_not_exist() {
    let temp_dir = tempdir().unwrap();

    let (update_socket, _query_runner) = init_app(&temp_dir, None);

    let owner_secret_key = AccountOwnerSecretKey::generate();
    let node_pub_key = NodeSecretKey::generate().to_pk();
    let locked_for = 365;
    let stake_lock_req = prepare_stake_lock_update(&node_pub_key, locked_for, &owner_secret_key, 1);

    expect_tx_revert(
        stake_lock_req,
        &update_socket,
        ExecutionError::NodeDoesNotExist,
    )
    .await;
}

#[tokio::test]
async fn test_stake_lock_reverts_not_node_owner() {
    let temp_dir = tempdir().unwrap();

    let (update_socket, query_runner) = init_app(&temp_dir, None);

    let owner_secret_key = AccountOwnerSecretKey::generate();
    let node_pub_key = NodeSecretKey::generate().to_pk();
    let amount: HpUfixed<18> = 1_000u64.into();

    deposit_and_stake(
        &update_socket,
        &owner_secret_key,
        1,
        &amount,
        &node_pub_key,
        [0; 96].into(),
    )
    .await;

    assert_eq!(get_staked(&query_runner, &node_pub_key), amount);

    let locked_for = 365;
    let stake_lock_req = prepare_stake_lock_update(
        &node_pub_key,
        locked_for,
        &AccountOwnerSecretKey::generate(),
        1,
    );

    expect_tx_revert(stake_lock_req, &update_socket, ExecutionError::NotNodeOwner).await;
}

#[tokio::test]
async fn test_stake_lock_reverts_insufficient_stake() {
    let temp_dir = tempdir().unwrap();

    let (update_socket, query_runner) = init_app(&temp_dir, None);

    let owner_secret_key = AccountOwnerSecretKey::generate();
    let node_pub_key = NodeSecretKey::generate().to_pk();
    let amount: HpUfixed<18> = 0u64.into();

    deposit_and_stake(
        &update_socket,
        &owner_secret_key,
        1,
        &amount,
        &node_pub_key,
        [0; 96].into(),
    )
    .await;

    assert_eq!(get_staked(&query_runner, &node_pub_key), amount);

    let locked_for = 365;
    let stake_lock_req = prepare_stake_lock_update(&node_pub_key, locked_for, &owner_secret_key, 3);

    expect_tx_revert(
        stake_lock_req,
        &update_socket,
        ExecutionError::InsufficientStake,
    )
    .await;
}

#[tokio::test]
async fn test_stake_lock_reverts_lock_exceeded_max_stake_lock_time() {
    let temp_dir = tempdir().unwrap();

    let (update_socket, query_runner) = init_app(&temp_dir, None);

    let owner_secret_key = AccountOwnerSecretKey::generate();
    let node_pub_key = NodeSecretKey::generate().to_pk();
    let amount: HpUfixed<18> = 1000u64.into();

    deposit_and_stake(
        &update_socket,
        &owner_secret_key,
        1,
        &amount,
        &node_pub_key,
        [0; 96].into(),
    )
    .await;

    assert_eq!(get_staked(&query_runner, &node_pub_key), amount);

    // max locked time from genesis
    let locked_for = 1460 + 1;
    let stake_lock_req = prepare_stake_lock_update(&node_pub_key, locked_for, &owner_secret_key, 3);

    expect_tx_revert(
        stake_lock_req,
        &update_socket,
        ExecutionError::LockExceededMaxStakeLockTime,
    )
    .await;
}

#[tokio::test]
async fn test_unstake_reverts_not_account_key() {
    let temp_dir = tempdir().unwrap();

    let committee_size = 4;
    let (committee, keystore) = create_genesis_committee(committee_size);
    let (update_socket, _query_runner) = test_init_app(&temp_dir, committee);

    let unstake = UpdateMethod::Unstake {
        amount: 100u64.into(),
        node: NodeSecretKey::generate().to_pk(),
    };

    // Check that trying to Unstake funds with Node Key reverts
    let node_secret_key = &keystore[0].node_secret_key;
    let update_node_key = prepare_update_request_node(unstake.clone(), node_secret_key, 1);
    expect_tx_revert(
        update_node_key,
        &update_socket,
        ExecutionError::OnlyAccountOwner,
    )
    .await;

    // Check that trying to Unstake funds with Consensus Key reverts
    let consensus_secret_key = &keystore[0].consensus_secret_key;
    let update_consensus_key = prepare_update_request_consensus(unstake, consensus_secret_key, 2);
    expect_tx_revert(
        update_consensus_key,
        &update_socket,
        ExecutionError::OnlyAccountOwner,
    )
    .await;
}

#[tokio::test]
async fn test_unstake_reverts_node_does_not_exist() {
    let temp_dir = tempdir().unwrap();

    let (update_socket, _query_runner) = init_app(&temp_dir, None);

    let owner_secret_key = AccountOwnerSecretKey::generate();
    let node_pub_key = NodeSecretKey::generate().to_pk();
    let update = prepare_unstake_update(&100u64.into(), &node_pub_key, &owner_secret_key, 1);

    expect_tx_revert(update, &update_socket, ExecutionError::NodeDoesNotExist).await;
}

#[tokio::test]
async fn test_unstake_reverts_insufficient_balance() {
    let temp_dir = tempdir().unwrap();

    let (update_socket, query_runner) = init_app(&temp_dir, None);

    let owner_secret_key = AccountOwnerSecretKey::generate();
    let node_pub_key = NodeSecretKey::generate().to_pk();
    let amount: HpUfixed<18> = 1_000u64.into();

    deposit_and_stake(
        &update_socket,
        &owner_secret_key,
        1,
        &amount,
        &node_pub_key,
        [0; 96].into(),
    )
    .await;

    assert_eq!(get_staked(&query_runner, &node_pub_key), amount);

    let update = prepare_unstake_update(
        &(amount + <u64 as Into<HpUfixed<18>>>::into(1)),
        &node_pub_key,
        &owner_secret_key,
        3,
    );

    expect_tx_revert(update, &update_socket, ExecutionError::InsufficientBalance).await;
}

#[tokio::test]
async fn test_revert_stake_not_account_key() {
    let temp_dir = tempdir().unwrap();

    let committee_size = 4;
    let (committee, keystore) = create_genesis_committee(committee_size);
    let (update_socket, _query_runner) = test_init_app(&temp_dir, committee);

    let amount: HpUfixed<18> = 1000_u64.into();

    let stake = UpdateMethod::Stake {
        amount,
        node_public_key: keystore[0].node_secret_key.to_pk(),
        consensus_key: None,
        node_domain: None,
        worker_public_key: None,
        worker_domain: None,
        ports: None,
    };

    // Check that trying to Stake funds with Node Key reverts
    let node_secret_key = &keystore[0].node_secret_key;
    let update_node_key = prepare_update_request_node(stake.clone(), node_secret_key, 1);
    expect_tx_revert(
        update_node_key,
        &update_socket,
        ExecutionError::OnlyAccountOwner,
    )
    .await;

    // Check that trying to Stake funds with Consensus Key reverts
    let consensus_secret_key = &keystore[0].consensus_secret_key;
    let update_consensus_key = prepare_update_request_consensus(stake, consensus_secret_key, 2);
    expect_tx_revert(
        update_consensus_key,
        &update_socket,
        ExecutionError::OnlyAccountOwner,
    )
    .await;
}

#[tokio::test]
async fn test_revert_stake_insufficient_balance() {
    let temp_dir = tempdir().unwrap();

    let committee_size = 4;
    let (committee, _keystore) = create_genesis_committee(committee_size);
    let (update_socket, query_runner) = test_init_app(&temp_dir, committee);

    let owner_secret_key = AccountOwnerSecretKey::generate();
    let address: EthAddress = owner_secret_key.to_pk().into();

    let peer_pub_key = NodeSecretKey::generate().to_pk();

    // Deposit some FLK into an account
    let amount = 1000_u64.into();
    deposit(&update_socket, &owner_secret_key, 1, &amount).await;

    let balance = get_flk_balance(&query_runner, &address);

    // Now try with the correct details for a new node
    let update = prepare_initial_stake_update(
        &(amount + <u64 as Into<HpUfixed<18>>>::into(1)),
        &peer_pub_key,
        [0; 96].into(),
        "127.0.0.1".parse().unwrap(),
        [0; 32].into(),
        "127.0.0.1".parse().unwrap(),
        NodePorts::default(),
        &owner_secret_key,
        2,
    );

    // Expect Revert Error
    expect_tx_revert(update, &update_socket, ExecutionError::InsufficientBalance).await;

    // Flk balance has not changed
    assert_eq!(get_flk_balance(&query_runner, &address), balance);
}

#[tokio::test]
async fn test_revert_stake_consensus_key_already_indexed() {
    let temp_dir = tempdir().unwrap();

    let committee_size = 4;
    let (committee, keystore) = create_genesis_committee(committee_size);
    let (update_socket, query_runner) = test_init_app(&temp_dir, committee);

    let owner_secret_key = AccountOwnerSecretKey::generate();
    let address: EthAddress = owner_secret_key.to_pk().into();

    let peer_pub_key = NodeSecretKey::generate().to_pk();

    // Deposit some FLK into an account
    let amount = 1000_u64.into();
    deposit(&update_socket, &owner_secret_key, 1, &amount).await;

    let balance = get_flk_balance(&query_runner, &address);

    // Now try with the correct details for a new node
    let update = prepare_initial_stake_update(
        &amount,
        &peer_pub_key,
        keystore[0].consensus_secret_key.to_pk(),
        "127.0.0.1".parse().unwrap(),
        [0; 32].into(),
        "127.0.0.1".parse().unwrap(),
        NodePorts::default(),
        &owner_secret_key,
        2,
    );

    // Expect Revert Error
    expect_tx_revert(
        update,
        &update_socket,
        ExecutionError::ConsensusKeyAlreadyIndexed,
    )
    .await;

    // Flk balance has not changed
    assert_eq!(get_flk_balance(&query_runner, &address), balance);
}

#[tokio::test]
async fn test_withdraw_unstaked_reverts_not_account_key() {
    let temp_dir = tempdir().unwrap();

    let committee_size = 4;
    let (committee, keystore) = create_genesis_committee(committee_size);
    let (update_socket, _query_runner) = test_init_app(&temp_dir, committee);

    let withdraw_unstaked = UpdateMethod::WithdrawUnstaked {
        node: NodeSecretKey::generate().to_pk(),
        recipient: None,
    };

    // Check that trying to Stake funds with Node Key reverts
    let node_secret_key = &keystore[0].node_secret_key;
    let update_node_key =
        prepare_update_request_node(withdraw_unstaked.clone(), node_secret_key, 1);
    expect_tx_revert(
        update_node_key,
        &update_socket,
        ExecutionError::OnlyAccountOwner,
    )
    .await;

    // Check that trying to Stake funds with Consensus Key reverts
    let consensus_secret_key = &keystore[0].consensus_secret_key;
    let update_consensus_key =
        prepare_update_request_consensus(withdraw_unstaked, consensus_secret_key, 2);
    expect_tx_revert(
        update_consensus_key,
        &update_socket,
        ExecutionError::OnlyAccountOwner,
    )
    .await;
}

#[tokio::test]
async fn test_withdraw_unstaked_reverts_node_does_not_exist() {
    let temp_dir = tempdir().unwrap();

    let (update_socket, _query_runner) = init_app(&temp_dir, None);

    let owner_secret_key = AccountOwnerSecretKey::generate();
    let node_pub_key = NodeSecretKey::generate().to_pk();
    let update = prepare_withdraw_unstaked_update(&node_pub_key, None, &owner_secret_key, 1);

    expect_tx_revert(update, &update_socket, ExecutionError::NodeDoesNotExist).await;
}

#[tokio::test]
async fn test_withdraw_unstaked_reverts_not_node_owner() {
    let temp_dir = tempdir().unwrap();

    let (update_socket, query_runner) = init_app(&temp_dir, None);

    let owner_secret_key = AccountOwnerSecretKey::generate();
    let node_pub_key = NodeSecretKey::generate().to_pk();
    let amount: HpUfixed<18> = 1_000u64.into();

    deposit_and_stake(
        &update_socket,
        &owner_secret_key,
        1,
        &amount,
        &node_pub_key,
        [0; 96].into(),
    )
    .await;

    assert_eq!(get_staked(&query_runner, &node_pub_key), amount);

    let withdraw_unstaked = prepare_withdraw_unstaked_update(
        &node_pub_key,
        None,
        &AccountOwnerSecretKey::generate(),
        1,
    );

    expect_tx_revert(
        withdraw_unstaked,
        &update_socket,
        ExecutionError::NotNodeOwner,
    )
    .await;
}

#[tokio::test]
async fn test_withdraw_unstaked_reverts_no_locked_tokens() {
    let temp_dir = tempdir().unwrap();

    let (update_socket, query_runner) = init_app(&temp_dir, None);

    let owner_secret_key = AccountOwnerSecretKey::generate();
    let node_pub_key = NodeSecretKey::generate().to_pk();
    let amount: HpUfixed<18> = 1_000u64.into();

    deposit_and_stake(
        &update_socket,
        &owner_secret_key,
        1,
        &amount,
        &node_pub_key,
        [0; 96].into(),
    )
    .await;

    assert_eq!(get_staked(&query_runner, &node_pub_key), amount);

    let withdraw_unstaked =
        prepare_withdraw_unstaked_update(&node_pub_key, None, &owner_secret_key, 3);

    expect_tx_revert(
        withdraw_unstaked,
        &update_socket,
        ExecutionError::NoLockedTokens,
    )
    .await;
}

#[tokio::test]
async fn test_withdraw_unstaked_works_properly() {
    let commit_phase_duration = 2000;
    let reveal_phase_duration = 2000;
    let mut network = TestNetwork::builder()
        .with_committee_beacon_config(CommitteeBeaconConfig::default())
        .with_mock_consensus(MockConsensusConfig {
            max_ordering_time: 0,
            min_ordering_time: 0,
            probability_txn_lost: 0.0,
            new_block_interval: Duration::from_millis(0),
            transactions_to_lose: Default::default(),
            block_buffering_interval: Duration::from_millis(0),
            forwarder_transaction_to_error: Default::default(),
        })
        .with_committee_nodes::<TestFullNodeComponentsWithMockConsensus>(4)
        .await
        .build()
        .await
        .unwrap();
    let node = network
        .node(0)
        .downcast::<TestFullNodeComponentsWithMockConsensus>();
    let node_initial_stake = node.get_stake();

    // Execute deposit and stake transasctions from the node owner account.
    let amount: HpUfixed<18> = 1_000u64.into();
    node.deposit_and_stake(amount.clone(), &node.get_owner_secret_key())
        .await
        .unwrap();

    // Check that node has the stake.
    assert_eq!(node.get_stake(), node_initial_stake + amount.clone());

    // Execute unstake transaction from node owner account.
    node.unstake(amount.clone(), &node.get_owner_secret_key())
        .await
        .unwrap();

    // Iterate through 5 epochs to to unlock `lock_time`.
    for _ in 0..5 {
        // Trigger epoch change and wait for it to be complete.
        network
            .change_epoch_and_wait_for_complete(0, commit_phase_duration, reveal_phase_duration)
            .await
            .unwrap();
    }

    // Get node owner flk balance.
    let prev_balance = node
        .app_query()
        .get_account_info(&node.get_owner_address(), |a| a.flk_balance)
        .unwrap();

    // Execute withdraw unstaked transaction.
    node.execute_transaction_from_owner(UpdateMethod::WithdrawUnstaked {
        node: node.get_node_public_key(),
        recipient: Some(node.get_owner_address()),
    })
    .await
    .unwrap();

    // Check that the flk balance was updated.
    assert_eq!(
        node.app_query()
            .get_account_info(&node.get_owner_address(), |a| a.flk_balance)
            .unwrap(),
        prev_balance + amount
    );

    // Check that the node's locked stake is reset.
    assert_eq!(
        node.app_query()
            .get_node_info::<HpUfixed<18>>(
                &node
                    .app_query()
                    .pubkey_to_index(&node.get_node_public_key())
                    .unwrap(),
                |n| n.stake.locked
            )
            .unwrap(),
        HpUfixed::zero()
    );

    // Shutdown the network.
    network.shutdown().await;
}

#[tokio::test]
async fn test_unstake_as_non_committee_node_opts_out_node_and_removes_after_epoch_change() {
    let network = utils::TestNetwork::builder()
        .with_committee_nodes(4)
        .with_non_committee_nodes(1)
        .build()
        .await
        .unwrap();
    let query = network.query();
    let epoch = query.get_current_epoch();

    // Check the initial stake.
    let stake = query.get_node_info(&4, |n| n.stake).unwrap();
    assert_eq!(stake.staked, 1000u64.into());
    assert_eq!(stake.locked, 0u64.into());

    // Execute unstake transaction from the first node.
    let resp = network
        .execute(vec![network.node(4).build_transasction_as_owner(
            UpdateMethod::Unstake {
                amount: 1000u64.into(),
                node: network.node(4).keystore.get_ed25519_pk(),
            },
            1,
        )])
        .await
        .unwrap();
    assert_eq!(resp.block_number, 1);

    // Check that the stake is now locked.
    let stake = query.get_node_info(&4, |n| n.stake).unwrap();
    assert_eq!(stake.staked, 0u64.into());
    assert_eq!(stake.locked, 1000u64.into());

    // Execute epoch change transactions.
    let resp = network.execute_change_epoch(epoch).await.unwrap();
    assert_eq!(resp.block_number, 2);

    // Execute commit-reveal transactions to complete the epoch change process.
    let resp = network
        .execute(
            network
                .nodes
                .iter()
                .enumerate()
                .take(4)
                .map(|(i, n)| {
                    n.build_transaction(UpdateMethod::CommitteeSelectionBeaconCommit {
                        commit: CommitteeSelectionBeaconCommit::build(epoch, 0, [i as u8; 32]),
                    })
                })
                .collect(),
        )
        .await
        .unwrap();
    assert_eq!(resp.block_number, 3);

    // Execute commit phase timeout transaction.
    let resp = network
        .execute(
            network
                .nodes
                .iter()
                .take(3)
                .map(|n| {
                    n.build_transaction(UpdateMethod::CommitteeSelectionBeaconCommitPhaseTimeout {
                        epoch,
                        round: 0,
                    })
                })
                .collect(),
        )
        .await
        .unwrap();
    assert_eq!(resp.block_number, 4);

    let resp = network
        .execute(
            network
                .nodes
                .iter()
                .enumerate()
                .take(4)
                .map(|(i, n)| {
                    n.build_transaction(UpdateMethod::CommitteeSelectionBeaconReveal {
                        reveal: [i as u8; 32],
                    })
                })
                .collect(),
        )
        .await
        .unwrap();
    assert_eq!(resp.block_number, 5);

    // Execute commit phase timeout transaction.
    let resp = network
        .execute(
            network
                .nodes
                .iter()
                .take(3)
                .map(|n| {
                    n.build_transaction(UpdateMethod::CommitteeSelectionBeaconRevealPhaseTimeout {
                        epoch,
                        round: 0,
                    })
                })
                .collect(),
        )
        .await
        .unwrap();
    assert_eq!(resp.block_number, 6);

    // Check that the epoch has changed.
    assert_eq!(query.get_current_epoch(), epoch + 1);

    // Check that the node is no longer participating.
    assert_eq!(
        query.get_node_info(&4, |n| n.participation).unwrap(),
        Participation::False
    );
}

#[tokio::test]
async fn test_unstake_as_committee_node_opts_out_node_and_removes_after_epoch_change() {
    let network = utils::TestNetwork::builder()
        .with_committee_nodes(5)
        .build()
        .await
        .unwrap();
    let query = network.query();
    let epoch = query.get_current_epoch();

    // Check the initial stake.
    let stake = query.get_node_info(&4, |n| n.stake).unwrap();
    assert_eq!(stake.staked, 1000u64.into());
    assert_eq!(stake.locked, 0u64.into());

    // Execute unstake transaction from the first node.
    let resp = network
        .execute(vec![network.node(4).build_transasction_as_owner(
            UpdateMethod::Unstake {
                amount: 1000u64.into(),
                node: network.node(4).keystore.get_ed25519_pk(),
            },
            1,
        )])
        .await
        .unwrap();
    assert_eq!(resp.block_number, 1);

    // Check that the stake is now locked.
    let stake = query.get_node_info(&4, |n| n.stake).unwrap();
    assert_eq!(stake.staked, 0u64.into());
    assert_eq!(stake.locked, 1000u64.into());

    // Execute epoch change transactions from participating nodes.
    let resp = network
        .execute(
            network.nodes[0..4]
                .iter()
                .map(|node| node.build_transaction(UpdateMethod::ChangeEpoch { epoch }))
                .collect(),
        )
        .await
        .unwrap();
    assert_eq!(resp.block_number, 2);

    // Execute commit-reveal transactions to complete the epoch change process.
    let resp = network
        .execute(
            network
                .nodes
                .iter()
                .enumerate()
                .map(|(i, n)| {
                    n.build_transaction(UpdateMethod::CommitteeSelectionBeaconCommit {
                        commit: CommitteeSelectionBeaconCommit::build(epoch, 0, [i as u8; 32]),
                    })
                })
                .collect(),
        )
        .await
        .unwrap();
    assert_eq!(resp.block_number, 3);

    // Execute commit phase timeout transaction.
    let resp = network
        .execute(
            network
                .nodes
                .iter()
                .take(4)
                .map(|n| {
                    n.build_transaction(UpdateMethod::CommitteeSelectionBeaconCommitPhaseTimeout {
                        epoch,
                        round: 0,
                    })
                })
                .collect(),
        )
        .await
        .unwrap();
    assert_eq!(resp.block_number, 4);

    let resp = network
        .execute(
            network
                .nodes
                .iter()
                .enumerate()
                .map(|(i, n)| {
                    n.build_transaction(UpdateMethod::CommitteeSelectionBeaconReveal {
                        reveal: [i as u8; 32],
                    })
                })
                .collect(),
        )
        .await
        .unwrap();
    assert_eq!(resp.block_number, 5);

    // Execute commit phase timeout transaction.
    let resp = network
        .execute(
            network
                .nodes
                .iter()
                .take(4)
                .map(|n| {
                    n.build_transaction(UpdateMethod::CommitteeSelectionBeaconRevealPhaseTimeout {
                        epoch,
                        round: 0,
                    })
                })
                .collect(),
        )
        .await
        .unwrap();
    assert_eq!(resp.block_number, 6);

    // Check that the epoch has changed.
    assert_eq!(query.get_current_epoch(), epoch + 1);

    // Check that the node is no longer participating.
    assert_eq!(
        query.get_node_info(&4, |n| n.participation).unwrap(),
        Participation::False
    );
}
