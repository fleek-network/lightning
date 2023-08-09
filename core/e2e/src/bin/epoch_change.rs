use std::time::{Duration, SystemTime};

use anyhow::Result;
use lightning_e2e::{swarm::Swarm, utils::rpc};
use resolve_path::PathResolveExt;
use serde_json::json;

// TODO(matthias): some of the RPC helpers are copied over from the rpc tests.
// We should probably make that code reusable.

#[tokio::main]
async fn main() -> Result<()> {
    // Start epoch now and let it end in 20 seconds.
    let epoch_start = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;

    let swarm = Swarm::builder()
        .with_directory("~/.fleek-test/e2e".resolve().into())
        .with_num_nodes(4)
        .with_epoch_time(20000)
        .with_epoch_start(epoch_start)
        .build();
    swarm.launch().await.unwrap();

    // Wait a bit for the nodes to start.
    tokio::time::sleep(Duration::from_secs(5)).await;

    let request = json!({
        "jsonrpc": "2.0",
        "method":"flk_get_epoch",
        "params":[],
        "id":1,
    });
    for (_, address) in swarm.get_rpc_addresses() {
        let response = rpc::rpc_request(address, request.to_string())
            .await
            .unwrap();

        let epoch = rpc::parse_response::<u64>(response)
            .await
            .expect("Failed to parse response.");
        assert_eq!(epoch, 0);
    }

    // The epoch will change after 20 seconds, and we already waited 5 seconds.
    // To give some time for the epoch change, we will wait another 30 seconds here.
    tokio::time::sleep(Duration::from_secs(20)).await;

    let request = json!({
        "jsonrpc": "2.0",
        "method":"flk_get_epoch",
        "params":[],
        "id":1,
    });
    for (_, address) in swarm.get_rpc_addresses() {
        let response = rpc::rpc_request(address, request.to_string())
            .await
            .unwrap();

        let epoch = rpc::parse_response::<u64>(response)
            .await
            .expect("Failed to parse response.");
        assert_eq!(epoch, 1);
    }

    println!("Epoch change: test passed");
    Ok(())
}
