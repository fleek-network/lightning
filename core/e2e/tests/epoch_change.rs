use std::time::{Duration, SystemTime};

use anyhow::Result;
use lightning_e2e::swarm::Swarm;
use lightning_e2e::utils::rpc;
use resolved_pathbuf::ResolvedPathBuf;
use serde_json::json;

#[tokio::test]
async fn e2e_epoch_change_all_nodes_on_committee() -> Result<()> {
    // Start epoch now and let it end in 40 seconds.
    let epoch_start = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;

    let path = ResolvedPathBuf::try_from("~/.lightning-test/e2e/epoch-change-committee").unwrap();
    let swarm = Swarm::builder()
        .with_directory(path)
        .with_min_port(10000)
        .with_max_port(10100)
        .with_num_nodes(4)
        .with_epoch_time(40000)
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

    // The epoch will change after 40 seconds, and we already waited 5 seconds.
    // To give some time for the epoch change, we will wait another 30 seconds here.
    tokio::time::sleep(Duration::from_secs(40)).await;

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
    Ok(())
}

//#[tokio::test]
//async fn e2e_epoch_change_x() -> Result<()> {
//    env_logger::init();
//    // Start epoch now and let it end in 40 seconds.
//    let epoch_start = SystemTime::now()
//        .duration_since(SystemTime::UNIX_EPOCH)
//        .unwrap()
//        .as_millis() as u64;
//
//    let path = ResolvedPathBuf::try_from("~/.lightning-test/e2e/epoch-change").unwrap();
//    let swarm = Swarm::builder()
//        .with_directory(path)
//        .with_min_port(10101)
//        .with_max_port(10200)
//        .with_num_nodes(5)
//        .with_committee_size(4)
//        .with_epoch_time(40000)
//        .with_epoch_start(epoch_start)
//        .build();
//    swarm.launch().await.unwrap();
//
//    // Wait a bit for the nodes to start.
//    tokio::time::sleep(Duration::from_secs(5)).await;
//
//    let request = json!({
//        "jsonrpc": "2.0",
//        "method":"flk_get_epoch",
//        "params":[],
//        "id":1,
//    });
//    for (_, address) in swarm.get_rpc_addresses() {
//        let response = rpc::rpc_request(address, request.to_string())
//            .await
//            .unwrap();
//
//        let epoch = rpc::parse_response::<u64>(response)
//            .await
//            .expect("Failed to parse response.");
//        assert_eq!(epoch, 0);
//    }
//
//    // The epoch will change after 40 seconds, and we already waited 5 seconds.
//    // To give some time for the epoch change, we will wait another 30 seconds here.
//    tokio::time::sleep(Duration::from_secs(40)).await;
//
//    let request = json!({
//        "jsonrpc": "2.0",
//        "method":"flk_get_epoch",
//        "params":[],
//        "id":1,
//    });
//    let mut count = 0;
//    for (key, address) in swarm.get_rpc_addresses() {
//        let response = rpc::rpc_request(address, request.to_string())
//            .await
//            .unwrap();
//
//        let epoch = rpc::parse_response::<u64>(response)
//            .await
//            .expect("Failed to parse response.");
//        // TODO(matthias): add assert statement back
//        //assert_eq!(epoch, 1);
//        if epoch == 1 {
//            count += 1;
//        }
//        println!("key: {}, epoch: {epoch}", key.to_base64());
//    }
//    assert_eq!(count, swarm.get_rpc_addresses().len());
//    Ok(())
//}
