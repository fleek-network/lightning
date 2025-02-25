use std::time::{Duration, SystemTime};

use fleek_blake3 as blake3;
use lightning_e2e::swarm::Swarm;
use lightning_interfaces::prelude::*;
use lightning_test_utils::logging;
use tempfile::tempdir;

#[tokio::test]
async fn e2e_syncronize_state() {
    logging::setup(None);

    let temp_dir = tempdir().unwrap();
    let mut swarm = Swarm::builder()
        .with_directory(temp_dir.path().to_path_buf().try_into().unwrap())
        .with_min_port(10600)
        .with_num_nodes(5)
        .with_committee_size(4)
        // We need to include enough time in this epoch time for the nodes to start up, or else it
        // begins the epoch change immediately when they do. We can even get into a situation where
        // another epoch change starts quickly after that, causing our expectation of epoch = 1
        // below to fail.
        .with_epoch_time(10000)
        .with_commit_phase_time(3000)
        .with_reveal_phase_time(3000)
        .with_epoch_start(
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
        )
        .with_syncronizer_delta(Duration::from_secs(5))
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

    // Start the node that is not on the genesis committee.
    swarm.launch_non_genesis_committee().await.unwrap();

    // Get the checkpoint receivers from the syncronizer for the node that is not on the genesis
    // committee.
    let (pubkey, syncronizer) = swarm.get_non_genesis_committee_syncronizer().pop().unwrap();

    // Wait for the syncronizer to detect that we are behind and send the checkpoint hash.
    let ckpt_hash = syncronizer.next_checkpoint_hash().await.unwrap();

    // Get the hash for this checkpoint from our blockstore. The syncronizer should have downloaded
    // it.
    let blockstore = swarm.get_blockstore(&pubkey).unwrap();
    let checkpoint = blockstore.read_all_to_vec(&ckpt_hash).await.unwrap();

    // Make sure the checkpoint matches the hash.
    let hash = blake3::hash(&checkpoint);
    assert_eq!(hash, ckpt_hash);

    swarm.shutdown().await;
}
