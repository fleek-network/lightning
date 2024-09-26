use std::collections::BTreeMap;

use fleek_crypto::{AccountOwnerSecretKey, ConsensusSecretKey, NodeSecretKey, SecretKey};
use hp_fixed::unsigned::HpUfixed;
use lightning_interfaces::types::{ExecutionData, ExecutionError, Metadata, UpdateMethod, Value};
use lightning_interfaces::SyncQueryRunnerInterface;
use lightning_utils::application::QueryRunnerExt;
use tempfile::tempdir;

use super::utils::*;

#[tokio::test]
async fn test_epoch_changed() {
    let temp_dir = tempdir().unwrap();

    // Create a genesis committee and seed the application state with it.
    let committee_size = 4;
    let (committee, keystore) = create_genesis_committee(committee_size);
    let (update_socket, query_runner) = test_init_app(&temp_dir, committee);
    let required_signals = calculate_required_signals(committee_size);

    let epoch = 0;
    let nonce = 1;

    // Have (required_signals - 1) say they are ready to change epoch
    // make sure the epoch doesn't change each time someone signals
    for node in keystore.iter().take(required_signals - 1) {
        // Make sure epoch didn't change
        let res = change_epoch(&update_socket, &node.node_secret_key, nonce, epoch).await;
        assert!(!res.change_epoch);
    }
    // check that the current epoch is still 0
    assert_eq!(query_runner.get_epoch_info().epoch, 0);

    // Have the last needed committee member signal the epoch change and make sure it changes
    let res = change_epoch(
        &update_socket,
        &keystore[required_signals].node_secret_key,
        nonce,
        epoch,
    )
    .await;
    assert!(res.change_epoch);

    // Query epoch info and make sure it incremented to new epoch
    assert_eq!(query_runner.get_epoch_info().epoch, 1);
}

#[tokio::test]
async fn test_change_epoch_reverts_account_key() {
    let temp_dir = tempdir().unwrap();

    let committee_size = 4;
    let (committee, _keystore) = create_genesis_committee(committee_size);
    let (update_socket, _query_runner) = test_init_app(&temp_dir, committee);

    // Account Secret Key
    let secret_key = AccountOwnerSecretKey::generate();

    let change_epoch = UpdateMethod::ChangeEpoch { epoch: 0 };

    let update = prepare_update_request_account(change_epoch, &secret_key, 1);
    expect_tx_revert(update, &update_socket, ExecutionError::OnlyNode).await;
}

#[tokio::test]
async fn test_change_epoch_reverts_node_does_not_exist() {
    let temp_dir = tempdir().unwrap();

    // Create a genesis committee and seed the application state with it.
    let committee_size = 4;
    let (committee, _keystore) = create_genesis_committee(committee_size);
    let (update_socket, _query_runner) = test_init_app(&temp_dir, committee);

    // Unknown Node Key (without Stake)
    let node_secret_key = NodeSecretKey::generate();
    let change_epoch = UpdateMethod::ChangeEpoch { epoch: 0 };

    let update = prepare_update_request_node(change_epoch, &node_secret_key, 1);
    expect_tx_revert(update, &update_socket, ExecutionError::NodeDoesNotExist).await;
}

#[tokio::test]
async fn test_change_epoch_reverts_insufficient_stake() {
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

    let change_epoch = UpdateMethod::ChangeEpoch { epoch: 0 };
    let update = prepare_update_request_node(change_epoch, &node_secret_key, 1);
    expect_tx_revert(update, &update_socket, ExecutionError::InsufficientStake).await;
}

#[tokio::test]
async fn test_epoch_change_reverts_epoch_already_changed() {
    let temp_dir = tempdir().unwrap();

    // Create a genesis committee and seed the application state with it.
    let committee_size = 4;
    let (committee, keystore) = create_genesis_committee(committee_size);
    let (update_socket, query_runner) = test_init_app(&temp_dir, committee);

    // call epoch change
    simple_epoch_change(&update_socket, &keystore, &query_runner, 0).await;
    assert_eq!(query_runner.get_epoch_info().epoch, 1);

    let change_epoch = UpdateMethod::ChangeEpoch { epoch: 0 };
    let update = prepare_update_request_node(change_epoch, &keystore[0].node_secret_key, 2);
    expect_tx_revert(update, &update_socket, ExecutionError::EpochAlreadyChanged).await;
}

#[tokio::test]
async fn test_epoch_change_reverts_epoch_has_not_started() {
    let temp_dir = tempdir().unwrap();

    // Create a genesis committee and seed the application state with it.
    let committee_size = 4;
    let (committee, keystore) = create_genesis_committee(committee_size);
    let (update_socket, _query_runner) = test_init_app(&temp_dir, committee);

    let change_epoch = UpdateMethod::ChangeEpoch { epoch: 1 };
    let update = prepare_update_request_node(change_epoch, &keystore[0].node_secret_key, 1);
    expect_tx_revert(update, &update_socket, ExecutionError::EpochHasNotStarted).await;
}

#[tokio::test]
async fn test_epoch_change_reverts_not_committee_member() {
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

    deposit_and_stake(
        &update_socket,
        &owner_secret_key,
        1,
        &minimum_stake_amount,
        &node_secret_key.to_pk(),
        [0; 96].into(),
    )
    .await;

    let change_epoch = UpdateMethod::ChangeEpoch { epoch: 0 };
    let update = prepare_update_request_node(change_epoch, &node_secret_key, 1);
    expect_tx_revert(update, &update_socket, ExecutionError::NotCommitteeMember).await;
}

#[tokio::test]
async fn test_epoch_change_reverts_already_signaled() {
    let temp_dir = tempdir().unwrap();

    // Create a genesis committee and seed the application state with it.
    let committee_size = 4;
    let (committee, keystore) = create_genesis_committee(committee_size);
    let (update_socket, _query_runner) = test_init_app(&temp_dir, committee);

    let change_epoch = UpdateMethod::ChangeEpoch { epoch: 0 };
    let update = prepare_update_request_node(change_epoch.clone(), &keystore[0].node_secret_key, 1);
    expect_tx_success(update, &update_socket, ExecutionData::None).await;

    // Second update
    let update = prepare_update_request_node(change_epoch, &keystore[0].node_secret_key, 2);
    expect_tx_revert(update, &update_socket, ExecutionError::AlreadySignaled).await;
}

#[tokio::test]
async fn test_distribute_rewards() {
    let temp_dir = tempdir().unwrap();

    let committee_size = 4;
    let (committee, keystore) = create_genesis_committee(committee_size);

    let max_inflation = 10;
    let protocol_part = 10;
    let node_part = 80;
    let service_part = 10;
    let boost = 4;
    let supply_at_genesis = 1_000_000;
    let (update_socket, query_runner) = init_app_with_params(
        &temp_dir,
        Params {
            epoch_time: None,
            max_inflation: Some(max_inflation),
            protocol_share: Some(protocol_part),
            node_share: Some(node_part),
            service_builder_share: Some(service_part),
            max_boost: Some(boost),
            supply_at_genesis: Some(supply_at_genesis),
        },
        Some(committee),
    );

    // get params for emission calculations
    let percentage_divisor: HpUfixed<18> = 100_u16.into();
    let supply_at_year_start: HpUfixed<18> = supply_at_genesis.into();
    let inflation: HpUfixed<18> = HpUfixed::from(max_inflation) / &percentage_divisor;
    let node_share = HpUfixed::from(node_part) / &percentage_divisor;
    let protocol_share = HpUfixed::from(protocol_part) / &percentage_divisor;
    let service_share = HpUfixed::from(service_part) / &percentage_divisor;

    let owner_secret_key1 = AccountOwnerSecretKey::generate();
    let node_secret_key1 = NodeSecretKey::generate();
    let owner_secret_key2 = AccountOwnerSecretKey::generate();
    let node_secret_key2 = NodeSecretKey::generate();

    let deposit_amount = 10_000_u64.into();
    let locked_for = 1460;
    // deposit FLK tokens and stake it
    deposit_and_stake(
        &update_socket,
        &owner_secret_key1,
        1,
        &deposit_amount,
        &node_secret_key1.to_pk(),
        [0; 96].into(),
    )
    .await;
    deposit_and_stake(
        &update_socket,
        &owner_secret_key2,
        1,
        &deposit_amount,
        &node_secret_key2.to_pk(),
        [1; 96].into(),
    )
    .await;
    stake_lock(
        &update_socket,
        &owner_secret_key2,
        3,
        &node_secret_key2.to_pk(),
        locked_for,
    )
    .await;

    // submit pods for usage
    let commodity_10 = 12_800;
    let commodity_11 = 3_600;
    let commodity_21 = 5000;
    let pod_10 = prepare_pod_request(commodity_10, 0, &node_secret_key1, 1);
    let pod_11 = prepare_pod_request(commodity_11, 1, &node_secret_key1, 2);
    let pod_21 = prepare_pod_request(commodity_21, 1, &node_secret_key2, 1);

    let node_1_usd = 0.1 * (commodity_10 as f64) + 0.2 * (commodity_11 as f64); // 2_000 in revenue
    let node_2_usd = 0.2 * (commodity_21 as f64); // 1_000 in revenue
    let reward_pool: HpUfixed<6> = (node_1_usd + node_2_usd).into();

    let node_1_proportion: HpUfixed<18> = HpUfixed::from(2000_u64) / HpUfixed::from(3000_u64);
    let node_2_proportion: HpUfixed<18> = HpUfixed::from(1000_u64) / HpUfixed::from(3000_u64);

    let service_proportions: Vec<HpUfixed<18>> = vec![
        HpUfixed::from(1280_u64) / HpUfixed::from(3000_u64),
        HpUfixed::from(1720_u64) / HpUfixed::from(3000_u64),
    ];

    // run the delivery ack transaction
    run_updates(vec![pod_10, pod_11, pod_21], &update_socket).await;

    // call epoch change that will trigger distribute rewards
    simple_epoch_change(&update_socket, &keystore, &query_runner, 0).await;

    // assert stable balances
    assert_eq!(
        get_stables_balance(&query_runner, &owner_secret_key1.to_pk().into()),
        HpUfixed::<6>::from(node_1_usd) * node_share.convert_precision()
    );
    assert_eq!(
        get_stables_balance(&query_runner, &owner_secret_key2.to_pk().into()),
        HpUfixed::<6>::from(node_2_usd) * node_share.convert_precision()
    );

    let total_share =
        &node_1_proportion * HpUfixed::from(1_u64) + &node_2_proportion * HpUfixed::from(4_u64);

    // calculate emissions per unit
    let emissions: HpUfixed<18> = (inflation * supply_at_year_start) / &365.0.into();
    let emissions_for_node = &emissions * &node_share;

    // assert flk balances node 1
    assert_eq!(
        // node_flk_balance1
        get_flk_balance(&query_runner, &owner_secret_key1.to_pk().into()),
        // node_flk_rewards1
        (&emissions_for_node * &node_1_proportion) / &total_share
    );

    // assert flk balances node 2
    assert_eq!(
        // node_flk_balance2
        get_flk_balance(&query_runner, &owner_secret_key2.to_pk().into()),
        // node_flk_rewards2
        (&emissions_for_node * (&node_2_proportion * HpUfixed::from(4_u64))) / &total_share
    );

    // assert protocols share
    let protocol_account = match query_runner.get_metadata(&Metadata::ProtocolFundAddress) {
        Some(Value::AccountPublicKey(s)) => s,
        _ => panic!("AccountPublicKey is set genesis and should never be empty"),
    };
    let protocol_balance = get_flk_balance(&query_runner, &protocol_account);
    let protocol_rewards = &emissions * &protocol_share;
    assert_eq!(protocol_balance, protocol_rewards);

    let protocol_stables_balance = get_stables_balance(&query_runner, &protocol_account);
    assert_eq!(
        &reward_pool * &protocol_share.convert_precision(),
        protocol_stables_balance
    );

    // assert service balances with service id 0 and 1
    for s in 0..2 {
        let service_owner = query_runner.get_service_info(&s).unwrap().owner;
        let service_balance = get_flk_balance(&query_runner, &service_owner);
        assert_eq!(
            service_balance,
            &emissions * &service_share * &service_proportions[s as usize]
        );
        let service_stables_balance = get_stables_balance(&query_runner, &service_owner);
        assert_eq!(
            service_stables_balance,
            &reward_pool
                * &service_share.convert_precision()
                * &service_proportions[s as usize].convert_precision()
        );
    }
}

#[tokio::test]
async fn test_supply_across_epoch() {
    let temp_dir = tempdir().unwrap();

    let committee_size = 4;
    let (mut committee, mut keystore) = create_genesis_committee(committee_size);

    let epoch_time = 100;
    let max_inflation = 10;
    let protocol_part = 10;
    let node_part = 80;
    let service_part = 10;
    let boost = 4;
    let supply_at_genesis = 1000000;
    let (update_socket, query_runner) = init_app_with_params(
        &temp_dir,
        Params {
            epoch_time: Some(epoch_time),
            max_inflation: Some(max_inflation),
            protocol_share: Some(protocol_part),
            node_share: Some(node_part),
            service_builder_share: Some(service_part),
            max_boost: Some(boost),
            supply_at_genesis: Some(supply_at_genesis),
        },
        Some(committee.clone()),
    );

    // get params for emission calculations
    let percentage_divisor: HpUfixed<18> = 100_u16.into();
    let supply_at_year_start: HpUfixed<18> = supply_at_genesis.into();
    let inflation: HpUfixed<18> = HpUfixed::from(max_inflation) / &percentage_divisor;
    let node_share = HpUfixed::from(node_part) / &percentage_divisor;
    let protocol_share = HpUfixed::from(protocol_part) / &percentage_divisor;
    let service_share = HpUfixed::from(service_part) / &percentage_divisor;

    let owner_secret_key = AccountOwnerSecretKey::generate();
    let node_secret_key = NodeSecretKey::generate();
    let consensus_secret_key = ConsensusSecretKey::generate();

    let deposit_amount = 10_000_u64.into();
    // deposit FLK tokens and stake it
    deposit_and_stake(
        &update_socket,
        &owner_secret_key,
        1,
        &deposit_amount,
        &node_secret_key.to_pk(),
        consensus_secret_key.to_pk(),
    )
    .await;

    // the index should be increment of whatever the size of genesis committee is, 5 in this case
    add_to_committee(
        &mut committee,
        &mut keystore,
        node_secret_key.clone(),
        consensus_secret_key.clone(),
        owner_secret_key.clone(),
        5,
    );

    // every epoch supply increase similar for simplicity of the test
    let _node_1_usd = 0.1 * 10000_f64;

    // calculate emissions per unit
    let emissions_per_epoch: HpUfixed<18> = (&inflation * &supply_at_year_start) / &365.0.into();

    let mut supply = supply_at_year_start;

    // 365 epoch changes to see if the current supply and year start suppply are ok
    for epoch in 0..365 {
        // add at least one transaction per epoch, so reward pool is not zero
        let nonce = get_node_nonce(&query_runner, &node_secret_key.to_pk());
        let pod_10 = prepare_pod_request(10000, 0, &node_secret_key, nonce + 1);
        expect_tx_success(pod_10, &update_socket, ExecutionData::None).await;

        // We have to submit uptime measurements to make sure nodes aren't set to
        // participating=false in the next epoch.
        // This is obviously tedious. The alternative is to deactivate the removal of offline nodes
        // for testing.
        for node in &keystore {
            let mut map = BTreeMap::new();
            let measurements = test_reputation_measurements(100);

            for peer in &keystore {
                if node.node_secret_key == peer.node_secret_key {
                    continue;
                }
                let _ = update_reputation_measurements(
                    &query_runner,
                    &mut map,
                    &peer.node_secret_key.to_pk(),
                    measurements.clone(),
                );
            }
            let nonce = get_node_nonce(&query_runner, &node.node_secret_key.to_pk()) + 1;

            submit_reputation_measurements(&update_socket, &node.node_secret_key, nonce, map).await;
        }

        let (_, new_keystore) = prepare_new_committee(&query_runner, &committee, &keystore);
        simple_epoch_change(&update_socket, &new_keystore, &query_runner, epoch).await;

        let supply_increase = &emissions_per_epoch * &node_share
            + &emissions_per_epoch * &protocol_share
            + &emissions_per_epoch * &service_share;

        let total_supply = match query_runner.get_metadata(&Metadata::TotalSupply) {
            Some(Value::HpUfixed(s)) => s,
            _ => panic!("TotalSupply is set genesis and should never be empty"),
        };

        supply += supply_increase;
        assert_eq!(total_supply, supply);

        if epoch == 364 {
            // the supply_year_start should update
            let supply_year_start = match query_runner.get_metadata(&Metadata::SupplyYearStart) {
                Some(Value::HpUfixed(s)) => s,
                _ => panic!("SupplyYearStart is set genesis and should never be empty"),
            };
            assert_eq!(total_supply, supply_year_start);
        }
    }
}
