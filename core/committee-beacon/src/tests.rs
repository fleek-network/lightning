use lightning_interfaces::types::UpdateMethod;
use lightning_interfaces::{
    CommitteeBeaconInterface,
    CommitteeBeaconQueryInterface,
    SyncQueryRunnerInterface,
};
use lightning_test_utils::e2e::{TestNetworkBuilder, TestNodeBuilder};
use tempfile::tempdir;

#[tokio::test]
async fn test_start_shutdown() {
    let temp_dir = tempdir().unwrap();
    let _node = TestNodeBuilder::new(temp_dir.path().to_path_buf())
        .build()
        .await
        .unwrap();
}

// TODO(snormore): Fill out this test coverage.

#[tokio::test]
async fn test_epoch_change_single_node() {
    let mut network = TestNetworkBuilder::new()
        .with_num_nodes(1)
        .build()
        .await
        .unwrap();
    let node = network.node(0);

    // Send epoch change transaction from all nodes.
    let epoch = network.change_epoch().await;

    // Check that beacon phase is set.
    // We don't check for commit phase specifically because we can't be sure it hasn't transitioned
    // to the reveal phase before checking.
    let phase = node.get_committee_selection_beacon_phase();
    assert!(phase.is_some());

    // Check that beacons are in app state.
    // These difficult to catch this at the right time with queries, so we just check that the
    // number is less than or equal to the number of nodes.
    let beacons = node.app_query.get_committee_selection_beacons();
    assert!(beacons.len() <= network.node_count());

    // Check that beacons are in local database.
    // These difficult to catch this at the right time with queries, so we just check that the
    // number is less than or equal to the number of nodes.
    let beacons = node.committee_beacon.query().get_beacons();
    assert!(beacons.len() <= network.node_count());

    // Wait for reveal phase to complete and beacon phase to be unset.
    network
        .wait_for_committee_selection_beacon_phase_unset()
        .await
        .unwrap();

    // Check that the epoch has been incremented.
    let new_epoch = node.get_epoch();
    assert_eq!(new_epoch, epoch);

    // Check that there are no node beacons (commits and reveals) in app state.
    let beacons = node.app_query.get_committee_selection_beacons();
    assert!(beacons.is_empty());

    // Clearing the beacons at epoch change is best-effort, since we can't guarantee that
    // the notification will be received or the listener will be running, in the case of a
    // deployment for example. This is fine, since the beacons will be cleared on the next
    // committee selection phase anyway, and we don't rely on it for correctness.
    let beacons = node.committee_beacon.query().get_beacons();
    assert!(beacons.len() <= network.node_count());

    // Shutdown the network.
    network.shutdown().await;
}

#[tokio::test]
async fn test_epoch_change_multiple_nodes() {
    let mut network = TestNetworkBuilder::new()
        .with_num_nodes(3)
        .build()
        .await
        .unwrap();
    let node = network.node(0);

    // Send epoch change transaction from all nodes.
    let epoch = network.change_epoch().await;

    // Check that beacon phase is set.
    // We don't check for commit phase specifically because we can't be sure it hasn't transitioned
    // to the reveal phase before checking.
    let phase = node.get_committee_selection_beacon_phase();
    assert!(phase.is_some());

    // Check that beacons are in app state.
    // It's difficult to catch this at the right time with queries, so we just check that the
    // number is less than or equal to the number of nodes.
    let beacons = node.app_query.get_committee_selection_beacons();
    assert!(beacons.len() <= network.node_count());

    // Check that beacons are in local database.
    // It's difficult to catch this at the right time with queries, so we just check that the
    // number is less than or equal to the number of nodes.
    let beacons = node.committee_beacon.query().get_beacons();
    assert!(beacons.len() <= network.node_count());

    // Wait for reveal phase to complete and beacon phase to be unset.
    network
        .wait_for_committee_selection_beacon_phase_unset()
        .await
        .unwrap();

    // Check that the epoch has been incremented.
    let new_epoch = node.get_epoch();
    assert_eq!(new_epoch, epoch);

    // Check that there are no node beacons (commits and reveals) in app state.
    let beacons = node.app_query.get_committee_selection_beacons();
    assert!(beacons.is_empty());

    // Clearing the beacons at epoch change is best-effort, since we can't guarantee that
    // the notification will be received or the listener will be running, in the case of a
    // deployment for example. This is fine, since the beacons will be cleared on the next
    // committee selection phase anyway, and we don't rely on it for correctness.
    let beacons = node.committee_beacon.query().get_beacons();
    assert!(beacons.len() <= network.node_count());

    // Shutdown the network.
    network.shutdown().await;
}

#[tokio::test]
async fn test_block_executed_in_waiting_phase_should_do_nothing() {
    let mut network = TestNetworkBuilder::new()
        .with_num_nodes(2)
        .build()
        .await
        .unwrap();
    let node = network.node(0);

    // Check beacon phase before submitting transaction.
    let phase = node.get_committee_selection_beacon_phase();
    assert!(phase.is_none());

    // Submit a transaction that does nothing except increment the node's nonce.
    node.execute_transaction_from_node(UpdateMethod::IncrementNonce {})
        .await
        .unwrap();

    // Check that beacon phase has not changed.
    let phase = node.get_committee_selection_beacon_phase();
    assert!(phase.is_none());

    // Check that there are no node beacons (commits and reveals) in app state.
    let beacons = node.app_query.get_committee_selection_beacons();
    assert!(beacons.is_empty());

    // Check that there are no beacons in our local database.
    let beacons = node.committee_beacon.query().get_beacons();
    assert!(beacons.is_empty());

    // Shutdown the network.
    network.shutdown().await;
}
