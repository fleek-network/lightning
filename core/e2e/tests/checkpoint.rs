//use std::fs;
//use std::time::{Duration, SystemTime};
//
//use anyhow::Result;
//use lightning_e2e::swarm::Swarm;
//use lightning_e2e::utils::{logging, rpc};
//use resolved_pathbuf::ResolvedPathBuf;
//use serde_json::json;
//use serial_test::serial;

//#[tokio::test]
//#[serial]
//async fn e2e_checkpoint() -> Result<()> {
//    logging::setup();
//
//    // Start epoch now and let it end in 40 seconds.
//    let epoch_start = SystemTime::now()
//        .duration_since(SystemTime::UNIX_EPOCH)
//        .unwrap()
//        .as_millis() as u64;
//
//    let path = ResolvedPathBuf::try_from("~/.lightning-test/e2e/checkpoint").unwrap();
//    if path.exists() {
//        fs::remove_dir_all(&path).expect("Failed to clean up swarm directory before test.");
//    }
//    let swarm = Swarm::builder()
//        .with_directory(path)
//        .with_min_port(10401)
//        .with_max_port(10500)
//        .with_num_nodes(4)
//        .with_epoch_time(40000)
//        .with_epoch_start(epoch_start)
//        .use_persistence()
//        .build();
//    swarm.launch().await.unwrap();
//
//    // Wait for the epoch to change.
//    tokio::time::sleep(Duration::from_secs(50)).await;
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
//        assert_eq!(epoch, 1);
//    }
//
//    swarm.shutdown();
//    Ok(())
//}
