use std::fs;
use std::time::{Duration, SystemTime};

use anyhow::Result;
use fleek_blake3 as blake3;
use lightning_e2e::swarm::Swarm;
use lightning_e2e::utils::rpc;
use lightning_interfaces::prelude::*;
use lightning_test_utils::logging;
use resolved_pathbuf::ResolvedPathBuf;
use serde_json::json;
use serial_test::serial;

#[tokio::test]
#[serial]
async fn e2e_syncronize_state() -> Result<()> {
    logging::setup();

    // Start epoch now and let it end in 40 seconds.
    let epoch_start = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;

    let path = ResolvedPathBuf::try_from("~/.lightning-test/e2e/syncronize-state").unwrap();
    if path.exists() {
        fs::remove_dir_all(&path).expect("Failed to clean up swarm directory before test.");
    }
    let swarm = Swarm::builder()
        .with_directory(path.clone())
        .with_min_port(10600)
        .with_num_nodes(5)
        .with_committee_size(4)
        .with_epoch_time(15000)
        .with_epoch_start(epoch_start)
        .with_syncronizer_delta(Duration::from_secs(5))
        .persistence(true)
        .build();
    swarm.launch_genesis_committee().await.unwrap();

    // Wait for the epoch to change.
    tokio::time::sleep(Duration::from_secs(20)).await;

    // Make sure that all genesis nodes changed epoch.
    let request = json!({
        "jsonrpc": "2.0",
        "method":"flk_get_epoch",
        "params":[],
        "id":1,
    });
    for (_, address) in swarm.get_genesis_committee_rpc_addresses() {
        let response = rpc::rpc_request(address, request.to_string())
            .await
            .unwrap();

        let epoch = rpc::parse_response::<u64>(response)
            .await
            .expect("Failed to parse response.");
        assert_eq!(epoch, 1);
    }

    // Start the node that is not on the genesis committee.
    swarm.launch_non_genesis_committee().await.unwrap();

    // Get the checkpoint receivers from the syncronizer for the node that is not on the genesis
    // committee.
    let (pubkey, syncronizer) = swarm.get_non_genesis_committee_syncronizer().pop().unwrap();

    // Wait for the syncronizer to detect that we are behind and send the checkpoint hash.
    let ckpt_hash = syncronizer.next_checkpoint_hash().await;

    // Get the hash for this checkpoint from our blockstore. The syncronizer should have downloaded
    // it.
    let blockstore = swarm.get_blockstore(&pubkey).unwrap();
    let checkpoint = blockstore.read_all_to_vec(&ckpt_hash).await.unwrap();

    // Make sure the checkpoint matches the hash.
    let hash = blake3::hash(&checkpoint);
    assert_eq!(hash, ckpt_hash);

    swarm.shutdown().await;

    Ok(())
}
