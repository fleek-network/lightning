use std::time::{Duration, SystemTime};

use anyhow::Result;
use fleek_crypto::NodePublicKey;
use lightning_e2e::{swarm::Swarm, utils::rpc};
use resolved_pathbuf::ResolvedPathBuf;
use serde_json::{json, Value};

fn rpc_request(method_name: &str, params: Option<Value>) -> Value {
    let actual_params = params.unwrap_or_else(|| Value::Array(Vec::new()));
    json!({
        "jsonrpc": "2.0",
        "method": method_name,
        "params": actual_params,
        "id":1,
    })
}

#[tokio::test]
async fn e2e_epoch_change() -> Result<()> {
    // Start epoch now and let it end in 20 seconds.
    let epoch_start = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;

    let path = ResolvedPathBuf::try_from("~/.fleek-test/e2e/epoch-change").unwrap();
    let swarm = Swarm::builder()
        .with_directory(path)
        .with_num_nodes(6)
        .with_epoch_time(20000)
        .with_epoch_start(epoch_start)
        .with_committee_size(4)
        .build();
    swarm.launch().await.unwrap();

    // Wait a bit for the nodes to start.
    tokio::time::sleep(Duration::from_secs(5)).await;

    let request = rpc_request("flk_get_epoch", None);

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

    let request = rpc_request("flk_get_epoch", None);
    let committee_request = rpc_request("flk_get_committee_members", None);

    let mut committee = Vec::new();
    for (index, (_, address)) in swarm.get_rpc_addresses().iter().enumerate() {
        let response = rpc::rpc_request(address.clone(), request.to_string())
            .await
            .unwrap();

        let epoch = rpc::parse_response::<u64>(response)
            .await
            .expect("Failed to parse response.");
        assert_eq!(epoch, 1);

        let committee_response =
            rpc::rpc_request(address.to_owned(), committee_request.to_string())
                .await
                .unwrap();

        let committee_members = rpc::parse_response::<Vec<NodePublicKey>>(committee_response)
            .await
            .expect("Failed to parse response.");
        if index == 0 {
            committee = committee_members.clone();
        }
        assert_eq!(committee, committee_members);
    }
    Ok(())
}
