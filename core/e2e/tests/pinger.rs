use std::time::{Duration, SystemTime};

use lightning_e2e::swarm::Swarm;
use lightning_interfaces::types::Participation;
use lightning_rpc::api::RpcClient;
use lightning_rpc::interface::Fleek;
use lightning_test_utils::logging;
use tempfile::tempdir;

#[tokio::test]
async fn e2e_detect_offline_node() {
    logging::setup(None);

    let temp_dir = tempdir().unwrap();
    let mut swarm = Swarm::builder()
        .with_directory(temp_dir.path().to_path_buf().try_into().unwrap())
        .with_min_port(10500)
        .with_num_nodes(5)
        .with_committee_size(4)
        // We need to include enough time in this epoch time for the nodes to start up and for some
        // pings to fail against the offline node, and for those reputation measurements to be
        // submitted, before the epoch change. Otherwise, the offline will still be marked as
        // participating = true and our expectation below will fail.
        .with_epoch_time(20000)
        .with_commit_phase_time(5000)
        .with_reveal_phase_time(5000)
        .with_ping_interval(Duration::from_secs(1))
        .with_ping_timeout(Duration::from_secs(1))
        .with_epoch_start(
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
        )
        .persistence(true)
        .build();
    swarm.launch_genesis_committee().await.unwrap();

    // Wait for RPC to be ready.
    swarm.wait_for_rpc_ready().await;

    // Wait for epoch to change.
    swarm
        .wait_for_epoch_change(1, Duration::from_secs(60))
        .await
        .unwrap();

    // Get the public key of the node that was offline.
    let (pubkey, _) = swarm
        .get_non_genesis_committee_rpc_addresses()
        .into_iter()
        .next()
        .unwrap();

    // Make sure that the offline node was removed from participation.
    for (_, address) in swarm.get_genesis_committee_rpc_addresses() {
        let client = RpcClient::new_no_auth(&address).unwrap();
        let node_info = client
            .get_node_info(pubkey, None)
            .await
            .unwrap()
            .expect("No node info recieved from rpc");

        assert_eq!(node_info.participation, Participation::False);
    }

    swarm.shutdown().await;
}
