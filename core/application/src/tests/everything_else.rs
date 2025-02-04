use std::cmp::Ordering;

use fleek_crypto::{AccountOwnerSecretKey, EthAddress, NodeSecretKey, SecretKey};
use hp_fixed::unsigned::HpUfixed;
use lightning_interfaces::prelude::*;
use lightning_interfaces::types::{
    ExecutionError,
    GenesisAccount,
    NodeIndex,
    NodeInfo,
    Participation,
    Staking,
    Tokens,
    UpdateMethod,
    UpdatePayload,
    UpdateRequest,
};
use lightning_interfaces::WithdrawPagingParams;
use lightning_utils::application::QueryRunnerExt;
use rand::seq::SliceRandom;
use tempfile::tempdir;

use super::utils::*;

#[tokio::test]
async fn test_has_sufficient_stake() {
    let temp_dir = tempdir().unwrap();

    let (update_socket, query_runner) = init_app(&temp_dir, None);

    let owner_secret_key = AccountOwnerSecretKey::generate();
    let node_pub_key = NodeSecretKey::generate().to_pk();

    // Stake minimum required amount.
    let minimum_stake_amount = query_runner.get_staking_amount().into();
    deposit_and_stake(
        &update_socket,
        &owner_secret_key,
        1,
        &minimum_stake_amount,
        &node_pub_key,
        [0; 96].into(),
    )
    .await;

    // Make sure that this node is a valid node.
    assert!(query_runner.has_sufficient_stake(&node_pub_key));

    // Generate new keys for a different node.
    let owner_secret_key = AccountOwnerSecretKey::generate();
    let node_pub_key = NodeSecretKey::generate().to_pk();

    // Stake less than the minimum required amount.
    let less_than_minimum_stake_amount = minimum_stake_amount / HpUfixed::<18>::from(2u16);
    deposit_and_stake(
        &update_socket,
        &owner_secret_key,
        1,
        &less_than_minimum_stake_amount,
        &node_pub_key,
        [1; 96].into(),
    )
    .await;
    // Make sure that this node is not a valid node.
    assert!(!query_runner.has_sufficient_stake(&node_pub_key));
}

#[tokio::test]
async fn test_simulate_txn() {
    let temp_dir = tempdir().unwrap();

    let committee_size = 4;
    let (committee, keystore) = create_genesis_committee(committee_size);
    let (update_socket, query_runner) = test_init_app(&temp_dir, committee);

    // Submit a ChangeEpoch transaction that will revert (EpochHasNotStarted) and ensure that the
    // `simulate_txn` method of the query runner returns the same response as the update runner.
    let invalid_epoch = 1;
    let req = prepare_change_epoch_request(invalid_epoch, &keystore[0].node_secret_key, 1);
    let res = run_update(req, &update_socket).await;

    let req = prepare_change_epoch_request(invalid_epoch, &keystore[0].node_secret_key, 2);
    assert_eq!(
        res.txn_receipts[0].response,
        query_runner.simulate_txn(req.into())
    );

    // Submit a ChangeEpoch transaction that will succeed and ensure that the
    // `simulate_txn` method of the query runner returns the same response as the update runner.
    let epoch = 0;
    let req = prepare_change_epoch_request(epoch, &keystore[0].node_secret_key, 2);

    let res = run_update(req, &update_socket).await;
    let req = prepare_change_epoch_request(epoch, &keystore[1].node_secret_key, 1);

    assert_eq!(
        res.txn_receipts[0].response,
        query_runner.simulate_txn(req.into())
    );
}

#[tokio::test]
async fn test_get_node_registry() {
    let temp_dir = tempdir().unwrap();

    let committee_size = 4;
    let (committee, keystore) = create_genesis_committee(committee_size);
    let (update_socket, query_runner) = test_init_app(&temp_dir, committee);

    let owner_secret_key1 = AccountOwnerSecretKey::generate();
    let node_secret_key1 = NodeSecretKey::generate();

    // Stake minimum required amount.
    let minimum_stake_amount = query_runner.get_staking_amount().into();
    deposit_and_stake(
        &update_socket,
        &owner_secret_key1,
        1,
        &minimum_stake_amount,
        &node_secret_key1.to_pk(),
        [0; 96].into(),
    )
    .await;

    // Generate new keys for a different node.
    let owner_secret_key2 = AccountOwnerSecretKey::generate();
    let node_secret_key2 = NodeSecretKey::generate();

    // Stake less than the minimum required amount.
    let less_than_minimum_stake_amount = minimum_stake_amount.clone() / HpUfixed::<18>::from(2u16);
    deposit_and_stake(
        &update_socket,
        &owner_secret_key2,
        1,
        &less_than_minimum_stake_amount,
        &node_secret_key2.to_pk(),
        [1; 96].into(),
    )
    .await;

    // Generate new keys for a different node.
    let owner_secret_key3 = AccountOwnerSecretKey::generate();
    let node_secret_key3 = NodeSecretKey::generate();

    // Stake minimum required amount.
    deposit(&update_socket, &owner_secret_key3, 1, &minimum_stake_amount).await;
    stake(
        &update_socket,
        &owner_secret_key3,
        2,
        &minimum_stake_amount,
        &node_secret_key3.to_pk(),
        [3; 96].into(),
    )
    .await;

    let valid_nodes = query_runner
        .get_node_registry(None)
        .into_iter()
        .map(|n| n.info)
        .collect::<Vec<NodeInfo>>();
    // We added two valid nodes, so the node registry should contain 2 nodes plus the committee.
    assert_eq!(valid_nodes.len(), 2 + keystore.len());
    assert_valid_node!(&valid_nodes, &query_runner, &node_secret_key1.to_pk());
    // Node registry doesn't contain the invalid node
    assert_not_valid_node!(&valid_nodes, &query_runner, &node_secret_key2.to_pk());
    assert_valid_node!(&valid_nodes, &query_runner, &node_secret_key3.to_pk());

    // We added 3 nodes, so the node registry should contain 3 nodes plus the committee.
    assert_paging_node_registry!(
        &query_runner,
        paging_params(true, 0, keystore.len() + 3),
        3 + keystore.len()
    );
    // We added 2 valid nodes, so the node registry should contain 2 nodes plus the committee.
    assert_paging_node_registry!(
        &query_runner,
        paging_params(false, 0, keystore.len() + 3),
        2 + keystore.len()
    );

    // We get the first 4 nodes.
    assert_paging_node_registry!(
        &query_runner,
        paging_params(true, 0, keystore.len()),
        keystore.len()
    );

    // The first 4 nodes are the committee and we added 3 nodes.
    assert_paging_node_registry!(&query_runner, paging_params(true, 4, keystore.len()), 3);

    // The first 4 nodes are the committee and we added 2 valid nodes.
    assert_paging_node_registry!(
        &query_runner,
        paging_params(false, keystore.len() as u32, keystore.len()),
        2
    );

    // The first 4 nodes are the committee and we added 3 nodes.
    assert_paging_node_registry!(
        &query_runner,
        paging_params(false, keystore.len() as u32, 1),
        1
    );
}

#[tokio::test]
async fn test_invalid_chain_id() {
    let temp_dir = tempdir().unwrap();

    let chain_id = CHAIN_ID + 1;
    let committee_size = 4;
    let (committee, keystore) = create_genesis_committee(committee_size);
    let (update_socket, _query_runner) = test_init_app(&temp_dir, committee);

    // Submit a OptIn transaction that will revert (InvalidChainID).

    // Regular Txn Execution
    let secret_key = &keystore[0].node_secret_key;
    let payload = UpdatePayload {
        sender: secret_key.to_pk().into(),
        nonce: 1,
        method: UpdateMethod::OptIn {},
        chain_id,
    };
    let digest = payload.to_digest();
    let signature = secret_key.sign(&digest);
    let update = UpdateRequest {
        signature: signature.into(),
        payload: payload.clone(),
    };
    expect_tx_revert(update, &update_socket, ExecutionError::InvalidChainId).await;
}

#[tokio::test]
async fn test_valid_mint_tx() {
    let temp_dir = tempdir().unwrap();

    let mut genesis = test_genesis();
    let gov_secret_key = AccountOwnerSecretKey::generate();
    let gov_public_key = gov_secret_key.to_pk();
    let gov_address: EthAddress = gov_public_key.into();

    genesis.governance_address = gov_address;

    let (update_socket, query_runner) = init_app_with_genesis(&temp_dir, &genesis);

    let secret_key = AccountOwnerSecretKey::generate();
    let public_key = secret_key.to_pk();
    let recv_address: EthAddress = public_key.into();

    let eth_tx_hash = [1; 32];

    let method = UpdateMethod::Mint {
        amount: HpUfixed::<18>::from(100_u64),
        token: Tokens::FLK,
        receiving_address: recv_address,
        eth_tx_hash,
        block_number: 1,
    };
    let request = prepare_update_request_account(method, &gov_secret_key, 1);

    // verify that the mint has not been ordered
    assert!(!query_runner.has_minted(eth_tx_hash));

    expect_tx_success(request, &update_socket, types::ExecutionData::None).await;

    // verify that the mint has been ordered
    assert!(query_runner.has_minted(eth_tx_hash));
}

#[tokio::test]
async fn test_double_mint_tx() {
    let temp_dir = tempdir().unwrap();

    let mut genesis = test_genesis();
    let gov_secret_key = AccountOwnerSecretKey::generate();
    let gov_public_key = gov_secret_key.to_pk();
    let gov_address: EthAddress = gov_public_key.into();

    genesis.governance_address = gov_address;

    let (update_socket, query_runner) = init_app_with_genesis(&temp_dir, &genesis);

    let secret_key = AccountOwnerSecretKey::generate();
    let public_key = secret_key.to_pk();
    let recv_address: EthAddress = public_key.into();

    let eth_tx_hash = [1; 32];

    let method = UpdateMethod::Mint {
        amount: HpUfixed::<18>::from(100_u64),
        token: Tokens::FLK,
        receiving_address: recv_address,
        eth_tx_hash,
        block_number: 1,
    };
    let request = prepare_update_request_account(method.clone(), &gov_secret_key, 1);

    // verify that the mint has not been ordered
    assert!(!query_runner.has_minted(eth_tx_hash));

    expect_tx_success(request, &update_socket, types::ExecutionData::None).await;

    // verify that the mint has been ordered
    assert!(query_runner.has_minted(eth_tx_hash));

    // try to mint again
    let request = prepare_update_request_account(method, &gov_secret_key, 2);
    expect_tx_revert(request, &update_socket, ExecutionError::AlreadyMinted).await;
}

#[tokio::test]
async fn test_mint_tx_non_governance_key() {
    let temp_dir = tempdir().unwrap();

    let (update_socket, query_runner) = init_app(&temp_dir, None);

    let secret_key = AccountOwnerSecretKey::generate();
    let public_key = secret_key.to_pk();
    let recv_address: EthAddress = public_key.into();

    let eth_tx_hash = [1; 32];

    let method = UpdateMethod::Mint {
        amount: HpUfixed::<18>::from(100_u64),
        token: Tokens::FLK,
        receiving_address: recv_address,
        eth_tx_hash,
        block_number: 1,
    };
    let request = prepare_update_request_account(method, &secret_key, 1);

    expect_tx_revert(request, &update_socket, ExecutionError::OnlyGovernance).await;

    // verify that the mint has not been ordered
    assert!(!query_runner.has_minted(eth_tx_hash));
}

#[tokio::test]
async fn test_clear_mints_tx_non_governance_key() {
    let temp_dir = tempdir().unwrap();

    let (update_socket, query_runner) = init_app(&temp_dir, None);

    let secret_key = AccountOwnerSecretKey::generate();

    let eth_tx_hash = [1; 32];

    let method = UpdateMethod::ClearMints { block_number: 1 };
    let request = prepare_update_request_account(method, &secret_key, 1);

    expect_tx_revert(request, &update_socket, ExecutionError::OnlyGovernance).await;

    // verify that the mint has not been ordered
    assert!(!query_runner.has_minted(eth_tx_hash));
}

#[tokio::test]
async fn test_valid_clear_mints_tx() {
    let temp_dir = tempdir().unwrap();

    let mut genesis = test_genesis();
    let gov_secret_key = AccountOwnerSecretKey::generate();
    let gov_public_key = gov_secret_key.to_pk();
    let gov_address: EthAddress = gov_public_key.into();

    genesis.governance_address = gov_address;

    let (update_socket, query_runner) = init_app_with_genesis(&temp_dir, &genesis);

    let secret_key = AccountOwnerSecretKey::generate();
    let public_key = secret_key.to_pk();
    let recv_address: EthAddress = public_key.into();
    let eth_tx_hash1 = [1; 32];
    let method = UpdateMethod::Mint {
        amount: HpUfixed::<18>::from(100_u64),
        token: Tokens::FLK,
        receiving_address: recv_address,
        eth_tx_hash: eth_tx_hash1,
        block_number: 1,
    };
    let request = prepare_update_request_account(method, &gov_secret_key, 1);
    expect_tx_success(request, &update_socket, types::ExecutionData::None).await;
    assert!(query_runner.has_minted(eth_tx_hash1));

    let secret_key = AccountOwnerSecretKey::generate();
    let public_key = secret_key.to_pk();
    let recv_address: EthAddress = public_key.into();
    let eth_tx_hash2 = [2; 32];
    let method = UpdateMethod::Mint {
        amount: HpUfixed::<18>::from(100_u64),
        token: Tokens::FLK,
        receiving_address: recv_address,
        eth_tx_hash: eth_tx_hash2,
        block_number: 2,
    };
    let request = prepare_update_request_account(method, &gov_secret_key, 2);
    expect_tx_success(request, &update_socket, types::ExecutionData::None).await;
    assert!(query_runner.has_minted(eth_tx_hash2));

    let secret_key = AccountOwnerSecretKey::generate();
    let public_key = secret_key.to_pk();
    let recv_address: EthAddress = public_key.into();
    let eth_tx_hash3 = [3; 32];
    let method = UpdateMethod::Mint {
        amount: HpUfixed::<18>::from(100_u64),
        token: Tokens::FLK,
        receiving_address: recv_address,
        eth_tx_hash: eth_tx_hash3,
        block_number: 3,
    };
    let request = prepare_update_request_account(method, &gov_secret_key, 3);
    expect_tx_success(request, &update_socket, types::ExecutionData::None).await;
    assert!(query_runner.has_minted(eth_tx_hash3));

    let method = UpdateMethod::ClearMints { block_number: 3 };
    let request = prepare_update_request_account(method, &gov_secret_key, 4);
    expect_tx_success(request, &update_socket, types::ExecutionData::None).await;

    // verify that tx1 and tx2 were removed from the table, because their block number is less than
    // 2.
    assert!(!query_runner.has_minted(eth_tx_hash1));
    assert!(!query_runner.has_minted(eth_tx_hash2));
    assert!(query_runner.has_minted(eth_tx_hash3));
}

#[tokio::test]
async fn test_clear_withdraws_tx_non_governance_key() {
    let temp_dir = tempdir().unwrap();

    let (update_socket, _query_runner) = init_app(&temp_dir, None);

    let secret_key = AccountOwnerSecretKey::generate();

    let method = UpdateMethod::ClearWithdraws { withdraw_id: 1 };
    let request = prepare_update_request_account(method, &secret_key, 1);

    expect_tx_revert(request, &update_socket, ExecutionError::OnlyGovernance).await;
}

#[tokio::test]
async fn test_valid_clear_withdraws_tx() {
    let temp_dir = tempdir().unwrap();

    let mut genesis = test_genesis();
    let gov_secret_key = AccountOwnerSecretKey::generate();
    let gov_public_key = gov_secret_key.to_pk();
    let gov_address: EthAddress = gov_public_key.into();

    genesis.governance_address = gov_address;

    let secret_key = AccountOwnerSecretKey::generate();
    let public_key = secret_key.to_pk();
    let recv_address: EthAddress = public_key.into();
    genesis.account = vec![GenesisAccount {
        bandwidth_balance: 0,
        flk_balance: 1000_u64.into(),
        stables_balance: 0,
        public_key: recv_address,
    }];

    let (update_socket, query_runner) = init_app_with_genesis(&temp_dir, &genesis);

    let method = UpdateMethod::Withdraw {
        amount: 100_u64.into(),
        token: Tokens::FLK,
        receiving_address: recv_address,
    };
    let request = prepare_update_request_account(method, &secret_key, 1);
    expect_tx_success(request, &update_socket, types::ExecutionData::None).await;

    let method = UpdateMethod::Withdraw {
        amount: 100_u64.into(),
        token: Tokens::FLK,
        receiving_address: recv_address,
    };
    let request = prepare_update_request_account(method, &secret_key, 2);
    expect_tx_success(request, &update_socket, types::ExecutionData::None).await;

    let method = UpdateMethod::Withdraw {
        amount: 100_u64.into(),
        token: Tokens::FLK,
        receiving_address: recv_address,
    };
    let request = prepare_update_request_account(method, &secret_key, 3);
    expect_tx_success(request, &update_socket, types::ExecutionData::None).await;

    let withdraws = query_runner.get_withdraws(WithdrawPagingParams {
        start: 0,
        limit: 10,
    });
    // Make sure all three withdraws are in the table
    assert_eq!(withdraws.len(), 3);
    // The withdraw ids start at 0, so at the beginning, the min id should be 0
    let min_id = withdraws.iter().map(|w| w.id).min().unwrap();
    assert_eq!(min_id, 0);

    let method = UpdateMethod::ClearWithdraws { withdraw_id: 2 };
    let request = prepare_update_request_account(method, &gov_secret_key, 1);
    expect_tx_success(request, &update_socket, types::ExecutionData::None).await;

    let withdraws = query_runner.get_withdraws(WithdrawPagingParams {
        start: 0,
        limit: 10,
    });
    // We removed all withdraws with id less than 2, which only leaves one withdraw
    assert_eq!(withdraws.len(), 1);
    // The min id should now be 2
    let min_id = withdraws.iter().map(|w| w.id).min().unwrap();
    assert_eq!(min_id, 2);
}

// (dalton) Since the quick sort used to select the winners of the auctions takes &self of the whole
// state, since it has to do reputation lookups on the compare nodes side of things I am going to
// repeate the modified quick sort algorithm here so we can have unit tests on just the actual
// algoritm
#[test]
fn test_quick_sort() {
    let nodes = quick_sort_mock_node_list();
    // We want the top 1000 nodes so the algorithm will find us and return the pivot from the bottom
    // 9000
    let k = 9000;
    let r = nodes.len() - 1;
    let winners = quick_sort_repeated(nodes, 0, r, k);

    assert_eq!(winners.len(), 1000);

    for node in winners {
        // Node indexes 9000-10000 should be the winners of the auction in this test
        assert!(node.0 > 8999);
    }
}

fn quick_sort_repeated(
    mut nodes: Vec<(NodeIndex, NodeInfo)>,
    l: usize,
    r: usize,
    k: usize,
) -> Vec<(NodeIndex, NodeInfo)> {
    let pivot = quick_sort_partition_repeated(&mut nodes, l, r);

    match pivot.cmp(&(k - 1)) {
        Ordering::Equal => nodes[pivot + 1..].to_vec(),
        Ordering::Greater => quick_sort_repeated(nodes, l, pivot - 1, k),
        _ => quick_sort_repeated(nodes, pivot + 1, r, k),
    }
}

fn quick_sort_partition_repeated(nodes: &mut [(NodeIndex, NodeInfo)], l: usize, r: usize) -> usize {
    let pivot = nodes[r].clone();
    let mut i = l;

    for j in l..r {
        if compare_nodes_repeated(&nodes[j].1, &pivot.1) {
            nodes.swap(j, i);
            i += 1;
        }
    }
    nodes.swap(i, r);

    i
}

fn compare_nodes_repeated(left: &NodeInfo, right: &NodeInfo) -> bool {
    left.stake.staked <= right.stake.staked
}

// Provides list of 10k nodes each one staking one more than the last
fn quick_sort_mock_node_list() -> Vec<(NodeIndex, NodeInfo)> {
    let mut nodes = Vec::with_capacity(10_000);
    for i in 0..10_000 {
        nodes.push((
            i,
            NodeInfo {
                owner: [0; 20].into(),
                public_key: [0; 32].into(),
                consensus_key: [0; 96].into(),
                staked_since: 0,
                stake: Staking {
                    staked: i.into(),
                    stake_locked_until: 0,
                    locked: HpUfixed::zero(),
                    locked_until: 0,
                },
                domain: [0, 0, 0, 0].into(),
                worker_domain: [0, 0, 0, 0].into(),
                worker_public_key: [0; 32].into(),
                participation: Participation::True,
                nonce: 0,
                ports: Default::default(),
            },
        ));
    }
    nodes.shuffle(&mut rand::thread_rng());
    nodes
}
