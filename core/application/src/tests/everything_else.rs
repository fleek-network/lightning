use std::cmp::Ordering;
use std::collections::BTreeSet;
use std::time::Duration;

use atomo::{AtomoBuilder, DefaultSerdeBackend};
use atomo_rocks::Options;
use fleek_crypto::{
    AccountOwnerSecretKey,
    ClientPublicKey,
    ConsensusPublicKey,
    EthAddress,
    NodePublicKey,
    NodeSecretKey,
    SecretKey,
};
use hp_fixed::unsigned::HpUfixed;
use lightning_interfaces::prelude::*;
use lightning_interfaces::types::{
    AccountInfo,
    Blake3Hash,
    Committee,
    CommitteeSelectionBeaconCommit,
    CommitteeSelectionBeaconReveal,
    CommodityTypes,
    Epoch,
    ExecutionError,
    Metadata,
    NodeIndex,
    NodeInfo,
    NodeServed,
    Participation,
    ProtocolParamKey,
    ProtocolParamValue,
    ReportedReputationMeasurements,
    Service,
    ServiceId,
    ServiceRevenue,
    Staking,
    TotalServed,
    TxHash,
    UpdateMethod,
    UpdatePayload,
    UpdateRequest,
    Value,
};
use lightning_utils::application::QueryRunnerExt;
use rand::seq::SliceRandom;
use resolved_pathbuf::ResolvedPathBuf;
use tempfile::tempdir;

use super::utils::*;
use crate::env::Env;
use crate::storage::AtomoStorageBuilder;
use crate::ApplicationConfig;

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

#[tokio::test]
async fn db_test() {
    let mut config_1 = ApplicationConfig::default();

    let mut config_2 = ApplicationConfig::default();

    config_1.db_path = Some(
        "~/Desktop/lgtn_test/app_db1"
            .try_into()
            .expect("Failed to resolve path"),
    );

    config_2.db_path = Some(
        "~/Desktop/lgtn_test/app_db2"
            .try_into()
            .expect("Failed to resolve path"),
    );

    let mut db_options = Options::default();
    db_options.create_if_missing(true);
    db_options.create_missing_column_families(true);

    let serialized_db1 = std::fs::read(
        ResolvedPathBuf::try_from(
            "/home/matthias/Desktop/checkpoints/stable-vinthill/epoch_state_serialized",
        )
        .unwrap(),
    )
    .unwrap();

    let hash_db1 = std::fs::read(
        ResolvedPathBuf::try_from(
            "/home/matthias/Desktop/checkpoints/stable-vinthill/epoch_state_hash",
        )
        .unwrap(),
    )
    .unwrap();

    let serialized_db2 = std::fs::read(
        ResolvedPathBuf::try_from(
            "/home/matthias/Desktop/checkpoints/stable-singapore/epoch_state_serialized",
        )
        .unwrap(),
    )
    .unwrap();

    let hash_db2 = std::fs::read(
        ResolvedPathBuf::try_from(
            "/home/matthias/Desktop/checkpoints/stable-singapore/epoch_state_hash",
        )
        .unwrap(),
    )
    .unwrap();

    let extra_tables = vec![];
    let db_builder1 =
        AtomoStorageBuilder::new(Some(config_1.db_path.as_ref().clone().unwrap().as_path()))
            .with_options(db_options.clone())
            .from_checkpoint(hash_db1.try_into().unwrap(), &serialized_db1, &extra_tables);

    let db_builder2 =
        AtomoStorageBuilder::new(Some(config_2.db_path.as_ref().clone().unwrap().as_path()))
            .with_options(db_options)
            .from_checkpoint(hash_db2.try_into().unwrap(), &serialized_db2, &extra_tables);

    let mut atomo1 = AtomoBuilder::<AtomoStorageBuilder, DefaultSerdeBackend>::new(db_builder1);
    atomo1 = atomo1
        .with_table::<Metadata, Value>("metadata")
        .with_table::<EthAddress, AccountInfo>("account")
        .with_table::<ClientPublicKey, EthAddress>("client_keys")
        .with_table::<NodeIndex, NodeInfo>("node")
        .with_table::<ConsensusPublicKey, NodeIndex>("consensus_key_to_index")
        .with_table::<NodePublicKey, NodeIndex>("pub_key_to_index")
        .with_table::<(NodeIndex, NodeIndex), Duration>("latencies")
        .with_table::<Epoch, Committee>("committee")
        .with_table::<ServiceId, Service>("service")
        .with_table::<ProtocolParamKey, ProtocolParamValue>("parameter")
        .with_table::<NodeIndex, Vec<ReportedReputationMeasurements>>("rep_measurements")
        .with_table::<NodeIndex, u8>("rep_scores")
        .with_table::<NodeIndex, u8>("submitted_rep_measurements")
        .with_table::<NodeIndex, NodeServed>("current_epoch_served")
        .with_table::<NodeIndex, NodeServed>("last_epoch_served")
        .with_table::<Epoch, TotalServed>("total_served")
        .with_table::<CommodityTypes, HpUfixed<6>>("commodity_prices")
        .with_table::<ServiceId, ServiceRevenue>("service_revenue")
        .with_table::<TxHash, ()>("executed_digests")
        .with_table::<NodeIndex, u8>("uptime")
        .with_table::<Blake3Hash, BTreeSet<NodeIndex>>("uri_to_node")
        .with_table::<NodeIndex, BTreeSet<Blake3Hash>>("node_to_uri")
        .with_table::<NodeIndex, (
            CommitteeSelectionBeaconCommit,
            Option<CommitteeSelectionBeaconReveal>,
        )>("committee_selection_beacon")
        .with_table::<NodeIndex, ()>("committee_selection_beacon_non_revealing_node")
        .with_table::<u64, (EthAddress, HpUfixed<18>)>("flk_withdraws")
        .with_table::<u64, (EthAddress, HpUfixed<6>)>("usdc_withdraws")
        .enable_iter("current_epoch_served")
        .enable_iter("rep_measurements")
        .enable_iter("submitted_rep_measurements")
        .enable_iter("rep_scores")
        .enable_iter("latencies")
        .enable_iter("node")
        .enable_iter("executed_digests")
        .enable_iter("uptime")
        .enable_iter("service_revenue")
        .enable_iter("uri_to_node")
        .enable_iter("node_to_uri")
        .enable_iter("committee_selection_beacon")
        .enable_iter("committee_selection_beacon_non_revealing_node")
        .enable_iter("flk_withdraws")
        .enable_iter("usdc_withdraws");

    let mut atomo2 = AtomoBuilder::<AtomoStorageBuilder, DefaultSerdeBackend>::new(db_builder2);
    atomo2 = atomo2
        .with_table::<Metadata, Value>("metadata")
        .with_table::<EthAddress, AccountInfo>("account")
        .with_table::<ClientPublicKey, EthAddress>("client_keys")
        .with_table::<NodeIndex, NodeInfo>("node")
        .with_table::<ConsensusPublicKey, NodeIndex>("consensus_key_to_index")
        .with_table::<NodePublicKey, NodeIndex>("pub_key_to_index")
        .with_table::<(NodeIndex, NodeIndex), Duration>("latencies")
        .with_table::<Epoch, Committee>("committee")
        .with_table::<ServiceId, Service>("service")
        .with_table::<ProtocolParamKey, ProtocolParamValue>("parameter")
        .with_table::<NodeIndex, Vec<ReportedReputationMeasurements>>("rep_measurements")
        .with_table::<NodeIndex, u8>("rep_scores")
        .with_table::<NodeIndex, u8>("submitted_rep_measurements")
        .with_table::<NodeIndex, NodeServed>("current_epoch_served")
        .with_table::<NodeIndex, NodeServed>("last_epoch_served")
        .with_table::<Epoch, TotalServed>("total_served")
        .with_table::<CommodityTypes, HpUfixed<6>>("commodity_prices")
        .with_table::<ServiceId, ServiceRevenue>("service_revenue")
        .with_table::<TxHash, ()>("executed_digests")
        .with_table::<NodeIndex, u8>("uptime")
        .with_table::<Blake3Hash, BTreeSet<NodeIndex>>("uri_to_node")
        .with_table::<NodeIndex, BTreeSet<Blake3Hash>>("node_to_uri")
        .with_table::<NodeIndex, (
            CommitteeSelectionBeaconCommit,
            Option<CommitteeSelectionBeaconReveal>,
        )>("committee_selection_beacon")
        .with_table::<NodeIndex, ()>("committee_selection_beacon_non_revealing_node")
        .with_table::<u64, (EthAddress, HpUfixed<18>)>("flk_withdraws")
        .with_table::<u64, (EthAddress, HpUfixed<6>)>("usdc_withdraws")
        .enable_iter("current_epoch_served")
        .enable_iter("rep_measurements")
        .enable_iter("submitted_rep_measurements")
        .enable_iter("rep_scores")
        .enable_iter("latencies")
        .enable_iter("node")
        .enable_iter("executed_digests")
        .enable_iter("uptime")
        .enable_iter("service_revenue")
        .enable_iter("uri_to_node")
        .enable_iter("node_to_uri")
        .enable_iter("committee_selection_beacon")
        .enable_iter("committee_selection_beacon_non_revealing_node")
        .enable_iter("flk_withdraws")
        .enable_iter("usdc_withdraws");

    atomo1.build().unwrap();
    atomo2.build().unwrap();

    let env_1 = Env::new(&config_1, None).unwrap();
    let env_2 = Env::new(&config_2, None).unwrap();

    let qr_1 = env_1.query_runner();
    let qr_2 = env_2.query_runner();

    let epoch1 = qr_1.get_epoch_info().epoch;
    let epoch2 = qr_2.get_epoch_info().epoch;
    println!("epoch1: {epoch1:?}");
    println!("epoch2: {epoch2:?}");

    let block_num1 = qr_1.get_block_number().unwrap();
    let block_num2 = qr_2.get_block_number().unwrap();
    println!("block_num1: {block_num1:?}");
    println!("block_num2: {block_num2:?}");

    println!();
    println!("node1:");
    let node1 = qr_1.get_node_info(&1, |x| x).unwrap();
    println!("node1: {node1:?}");
    let node2 = qr_2.get_node_info(&1, |x| x).unwrap();
    println!("node2: {node2:?}");

    println!();
    println!("committee at epoch 34:");
    let comm1 = qr_1.get_committee_info(&34, |x| x);
    println!("comm1: {comm1:?}");
    let comm2 = qr_2.get_committee_info(&34, |x| x);
    println!("comm2: {comm2:?}");

    println!();
    println!("committee at epoch 35:");
    let comm1 = qr_1.get_committee_info(&35, |x| x);
    println!("comm1: {comm1:?}");
    let comm2 = qr_2.get_committee_info(&35, |x| x);
    println!("comm2: {comm2:?}");

    //let nodes = qr_2.get_node_registry(None);
    //for node in nodes {
    //    let eth_address =
    //        EthAddress::from_str("0x5b84350a65C7e353B5207fdE889C91e249413d85").unwrap();
    //    if node.owner == eth_address {
    //        println!("{node:?}");
    //    }
    //}

    //let path = "/home/matthias/Desktop/lat.csv";

    //let output = File::create(path).unwrap();
    //let lat = qr_1.get_latencies();

    //let mut writer = BufWriter::new(output);

    //for ((key1, key2), val) in lat {
    //    writer
    //        .write_all(format!("{},{},{}\n", key1, key2, val.as_millis()).as_bytes())
    //        .unwrap();
    //}
    //writer.flush().unwrap();
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
