use std::time::{Duration, SystemTime};

use lightning_e2e::swarm::Swarm;
use lightning_interfaces::BlockstoreInterface;
use lightning_node_bindings::FullNodeComponents;
use lightning_rpc::interface::Fleek;
use lightning_rpc::RpcClient;
use lightning_test_utils::logging;
use lightning_utils::poll::{poll_until, PollUntilError};
use tempfile::tempdir;

#[tokio::test]
async fn e2e_checkpoint() {
    logging::setup(None);

    let temp_dir = tempdir().unwrap();
    let mut swarm = Swarm::<FullNodeComponents>::builder()
        .with_directory(temp_dir.path().to_path_buf().try_into().unwrap())
        .with_min_port(10000)
        .with_num_nodes(4)
        // We need to include enough time in this epoch time for the nodes to start up, or else it
        // begins the epoch change immediately when they do. We can even get into a situation where
        // another epoch change starts quickly after that, causing our expectation of epoch = 1
        // below to fail.
        .with_epoch_time(15000)
        .with_commit_phase_time(3000)
        .with_reveal_phase_time(3000)
        .with_epoch_start(
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
        )
        .persistence(true)
        .build::<FullNodeComponents>();
    swarm.launch().await.unwrap();

    // Wait for RPC to be ready.
    swarm.wait_for_rpc_ready().await;

    // Wait for the epoch to change.
    swarm
        .wait_for_epoch_change(1, Duration::from_secs(60))
        .await
        .unwrap();

    // Wait until the last epoch hash is not all zeroes and equal across all nodes.
    // We need to do this because the last epoch hash is not updated atomically with the epoch
    // change, and happens immediately after it, but we may check before it's been updated, so we
    // need to wait/poll for a short period just in case.
    let epoch_checkpoint_hash = poll_until(
        || async {
            // Check last epoch hash across all nodes.
            let mut target_hash = None;
            for (_, address) in swarm.get_rpc_addresses() {
                let client = RpcClient::new_no_auth(&address).unwrap();
                let (epoch_hash, _) = client.get_last_epoch_hash().await.unwrap();
                if target_hash.is_none() {
                    target_hash = Some(epoch_hash);
                }
                if epoch_hash != target_hash.unwrap() {
                    return Err(PollUntilError::ConditionNotSatisfied);
                }
            }
            let target_hash = target_hash.unwrap();

            // Check that the epoch hash is not all zeros, which would indicate that the checkpoint
            // was not stored, since that's the default.
            (target_hash != [0; 32])
                .then_some(target_hash)
                .ok_or(PollUntilError::ConditionNotSatisfied)
        },
        Duration::from_secs(5),
        Duration::from_millis(100),
    )
    .await
    .unwrap();

    // Check that the epoch hash is not all zeros, which would indicate that the checkpoint
    // was not stored, since that's the default.
    assert_ne!(epoch_checkpoint_hash, [0; 32]);

    // Get the checkpoint from blockstores across all nodes and make sure they all match the
    // expected checkpoint hash.
    for blockstore in swarm.get_blockstores() {
        let checkpoint = blockstore
            .read_all_to_vec(&epoch_checkpoint_hash)
            .await
            .unwrap();
        let checkpoint_hash = fleek_blake3::hash(&checkpoint);
        assert_eq!(checkpoint_hash, epoch_checkpoint_hash);
    }

    swarm.shutdown().await;
}
