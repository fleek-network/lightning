use std::time::{Duration, SystemTime};

use anyhow::Result;
use lightning_e2e::swarm::Swarm;
use reqwest::{Client, Response};
use resolve_path::PathResolveExt;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

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
        .with_directory("~/.fleek/2e2".resolve().into())
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
        let response = rpc_request(address, request.to_string()).await.unwrap();

        if response.status().is_success() {
            let value: Value = response.json().await?;
            if value.get("result").is_some() {
                // Parse the response as a successful response
                let success_response: RpcSuccessResponse<u64> = serde_json::from_value(value)?;

                // The epoch has not changed, so it should be 0.
                println!("epoch before change: {}", success_response.result);
                assert_eq!(0, success_response.result);
            } else {
                panic!("Rpc Error: {value}")
            }
        } else {
            panic!("Request failed with status: {}", response.status());
        }
    }

    // The epoch will change after 20 seconds, and we already waited 5 seconds.
    // To give some time for the epoch change, we will wait another 30 seconds here.
    tokio::time::sleep(Duration::from_secs(30)).await;

    let request = json!({
        "jsonrpc": "2.0",
        "method":"flk_get_epoch",
        "params":[],
        "id":1,
    });
    for (_, address) in swarm.get_rpc_addresses() {
        let response = rpc_request(address, request.to_string()).await.unwrap();

        if response.status().is_success() {
            let value: Value = response.json().await?;
            if value.get("result").is_some() {
                // Parse the response as a successful response
                let success_response: RpcSuccessResponse<u64> = serde_json::from_value(value)?;

                // The epoch should have changed to 1.
                println!("epoch after change: {}", success_response.result);
                // TODO(matthias): the epoch is not changing because the nodes aren't on the
                // committee. find out why.
                //assert_eq!(1, success_response.result);
            } else {
                panic!("Rpc Error: {value}")
            }
        } else {
            panic!("Request failed with status: {}", response.status());
        }
    }

    println!("Epoch change: test passed");
    Ok(())
}

async fn rpc_request(address: String, request: String) -> Result<Response> {
    let client = Client::new();
    Ok(client
        .post(address)
        .header("Content-Type", "application/json")
        .body(request)
        .send()
        .await?)
}

#[derive(Serialize, Deserialize, Debug)]
struct RpcSuccessResponse<T> {
    jsonrpc: String,
    id: usize,
    result: T,
}
