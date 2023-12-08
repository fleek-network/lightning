// use std::fs;
// use std::time::{Duration, SystemTime};

// use anyhow::Result;
// use lightning_application::app::Application;
// use lightning_dht::Dht;
// use lightning_e2e::swarm::Swarm;
// use lightning_e2e::utils::logging;
// use lightning_interfaces::infu_collection::Collection;
// use lightning_interfaces::types::{Blake3Hash, KeyPrefix};
// use lightning_interfaces::{partial, DhtInterface};
// use lightning_node::FinalTypes;
// use lightning_topology::Topology;
// use resolved_pathbuf::ResolvedPathBuf;
// use serial_test::serial;

// partial!(PartialBinding {
//     ApplicationInterface = Application<Self>;
//     TopologyInterface = Topology<Self>;
//     DhtInterface = Dht<Self>;
// });

// #[tokio::test]
// #[serial]
// async fn e2e_dht_put_and_get() -> Result<()> {
//     logging::setup();

//     // Start epoch now and let it end in 20 seconds.
//     let epoch_start = SystemTime::now()
//         .duration_since(SystemTime::UNIX_EPOCH)
//         .unwrap()
//         .as_millis() as u64;

//     let path = ResolvedPathBuf::try_from("~/.lightning-test/e2e/dht").unwrap();
//     if path.exists() {
//         fs::remove_dir_all(&path).expect("Failed to clean up swarm directory before test.");
//     }
//     let swarm = Swarm::builder()
//         .with_directory(path)
//         .with_min_port(10301)
//         .with_max_port(10400)
//         .with_num_nodes(4)
//         .with_epoch_start(epoch_start)
//         .with_bootstrap_node()
//         .build();
//     swarm.launch_bootstrap_nodes().await.unwrap();

//     // Wait a bit for the nodes to start.
//     tokio::time::sleep(Duration::from_secs(2)).await;

//     swarm.launch_non_bootstrap_nodes().await.unwrap();

//     tokio::time::sleep(Duration::from_secs(5)).await;

//     let key: Blake3Hash = rand::random();
//     let value: [u8; 4] = rand::random();

//     #[allow(clippy::filter_map_identity)]
//     let dht_handles: Vec<Dht<FinalTypes>> = swarm
//         .get_dht_handle()
//         .into_iter()
//         .filter_map(|s| s)
//         .collect();

//     // Send DHT put to an arbitrary node in the swarm
//     dht_handles[1].put(KeyPrefix::ContentRegistry, key.as_ref(), value.as_ref());

//     // Wait some time for the DHT to do its magic
//     tokio::time::sleep(Duration::from_secs(15)).await;

//     // Perform a DHT lookup on every node in the swarm
//     for dht_handle in dht_handles {
//         let res = dht_handle
//             .get(KeyPrefix::ContentRegistry, key.as_ref())
//             .await;
//         match res {
//             Some(entry) => {
//                 // Make sure the retrieved value equals the value we stored
//                 assert_eq!(value.to_vec(), entry.value);
//             },
//             _ => panic!("Unexpected response"),
//         }
//     }

//     swarm.shutdown();
//     Ok(())
// }
