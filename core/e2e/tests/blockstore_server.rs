use std::time::{Duration, SystemTime};

use fleek_blake3 as blake3;
use fleek_crypto::NodePublicKey;
use lightning_blockstore::blockstore::BLOCK_SIZE;
use lightning_e2e::swarm::Swarm;
use lightning_interfaces::prelude::*;
use lightning_interfaces::types::{CompressionAlgorithm, ServerRequest};
use lightning_test_utils::logging;
use tempfile::tempdir;

fn create_content() -> Vec<u8> {
    (0..4)
        .map(|i| Vec::from([i; BLOCK_SIZE]))
        .flat_map(|a| a.into_iter())
        .collect()
}

#[tokio::test]
async fn e2e_blockstore_server_get() {
    logging::setup();

    let temp_dir = tempdir().unwrap();
    let mut swarm = Swarm::builder()
        .with_directory(temp_dir.path().to_path_buf().try_into().unwrap())
        .with_min_port(10700)
        .with_num_nodes(4)
        // We need to include enough time in this epoch time for the nodes to start up, or else it
        // begins the epoch change immediately when they do. We can even get into a situation where
        // another epoch change starts quickly after that, causing our expectation of epoch = 1
        // below to fail.
        .with_epoch_time(10000)
        .with_epoch_start(
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
        )
        .with_syncronizer_delta(Duration::from_secs(5))
        .build();
    swarm.launch().await.unwrap();

    // Wait for RPC to be ready.
    swarm.wait_for_rpc_ready().await;

    let pubkeys: Vec<NodePublicKey> = swarm.get_ports().keys().cloned().collect();
    let pubkey1 = pubkeys[0];
    let pubkey2 = pubkeys[1];
    let index1 = swarm.get_node_index(&pubkey1).unwrap();

    // Put some data into the blockstore of node1
    let data = create_content();
    let blockstore1 = swarm.get_blockstore(&pubkey1).unwrap();
    let mut putter = blockstore1.put(None);
    putter
        .write(data.as_slice(), CompressionAlgorithm::Uncompressed)
        .unwrap();
    let data_hash = putter.finalize().await.unwrap();

    // Send a request from node2 to node1 to obtain the data
    let blockstore2 = swarm.get_blockstore(&pubkey2).unwrap();
    let blockstore_server_socket2 = swarm.get_blockstore_server_socket(&pubkey2).unwrap();

    let mut res = blockstore_server_socket2
        .run(ServerRequest {
            hash: data_hash,
            peer: index1,
        })
        .await
        .expect("Failed to send request");
    match res.recv().await.unwrap() {
        Ok(()) => {
            // Make sure the data matches
            let recv_data = blockstore2.read_all_to_vec(&data_hash).await.unwrap();
            assert_eq!(data, recv_data);
            let hash = blake3::hash(&recv_data);
            assert_eq!(hash, data_hash);
        },
        Err(e) => panic!("Failed to receive content: {e:?}"),
    }

    swarm.shutdown().await;
}
