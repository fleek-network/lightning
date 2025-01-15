use std::time::{Duration, SystemTime};

use b3fs::entry::*;
use fleek_blake3 as blake3;
use fleek_crypto::NodePublicKey;
use futures::StreamExt;
use lightning_blockstore::blockstore::BLOCK_SIZE;
use lightning_e2e::swarm::Swarm;
use lightning_interfaces::prelude::*;
use lightning_interfaces::types::ServerRequest;
use lightning_interfaces::{_DirTrustedWriter, _FileTrustedWriter};
use lightning_origin_ipfs::config::{Gateway, Protocol, RequestFormat};
use lightning_test_utils::logging;
use lightning_test_utils::server::spawn_server;
use tempfile::tempdir;

use self::types::{
    FetcherRequest,
    FetcherResponse,
    ImmutablePointer,
    OriginProvider,
    ServerResponse,
};

fn create_content() -> Vec<u8> {
    (0..4)
        .map(|i| Vec::from([i; BLOCK_SIZE]))
        .flat_map(|a| a.into_iter())
        .collect()
}

#[tokio::test]
async fn e2e_blockstore_server_get() {
    logging::setup(None);

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
    let mut putter = blockstore1.file_writer().await.unwrap();
    putter.write(data.as_slice(), true).await.unwrap();
    let data_hash = putter.commit().await.unwrap();

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
        Ok(response) => {
            // Make sure the data matches
            let recv_data = blockstore2.read_all_to_vec(&data_hash).await.unwrap();
            assert_eq!(data, recv_data);
            let hash = blake3::hash(&recv_data);
            assert_eq!(hash, data_hash);
            match response {
                ServerResponse::Continue(_) => panic!("Unexpected. Only file is expected"),
                ServerResponse::EoR => (),
            }
        },
        Err(e) => panic!("Failed to receive content: {e:?}"),
    }

    swarm.shutdown().await;
}

#[tokio::test]
async fn e2e_blockstore_server_with_fetcher() {
    logging::setup(None);

    let temp_dir = tempdir().unwrap();
    let port_ipfs = spawn_server(10900).unwrap();

    let gateways = vec![Gateway {
        protocol: Protocol::Http,
        authority: format!("127.0.0.1:{}", port_ipfs),
        request_format: RequestFormat::CidLast,
    }];

    let mut swarm = Swarm::builder()
        .with_directory(temp_dir.path().to_path_buf().try_into().unwrap())
        .with_min_port(10800)
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
        .with_ipfs_gateways(gateways)
        .build();
    swarm.launch().await.unwrap();

    // Wait for RPC to be ready.
    swarm.wait_for_rpc_ready().await;

    let pubkeys: Vec<NodePublicKey> = swarm.get_ports().keys().cloned().collect();
    let pubkey1 = pubkeys[0];
    let pubkey2 = pubkeys[1];

    let cid =
        cid::Cid::try_from("bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi").unwrap();

    // Fetch data from IPFS in node 0
    let fetcher = swarm.get_fetcher_socket(&pubkey1).unwrap();
    let res = fetcher
        .run(FetcherRequest::Put {
            pointer: ImmutablePointer {
                origin: OriginProvider::IPFS,
                uri: cid.to_bytes(),
            },
        })
        .await
        .expect("Failed to send request");

    let hash = match res {
        FetcherResponse::Put(hash) => match hash {
            Ok(hash) => hash,
            Err(e) => panic!("Error getting hash {e:?}"),
        },
        FetcherResponse::Fetch(_) => panic!("impossible"),
    };

    tokio::time::sleep(Duration::from_secs(5)).await;
    // Fetch data from Node 1 to force getting from the other node 0
    let fetcher = swarm.get_fetcher_socket(&pubkey2).unwrap();
    let res = fetcher
        .run(FetcherRequest::Fetch { hash })
        .await
        .expect("Failed to send request");

    match res {
        FetcherResponse::Put(_) => panic!("impossible"),
        FetcherResponse::Fetch(hash_new) => {
            if let Err(err) = hash_new {
                panic!("Error {err:?}");
            }
        },
    }

    swarm.shutdown().await;
}

#[tokio::test]
async fn e2e_blockstore_server_with_fetcher_recursive_dir() {
    logging::setup(None);

    let temp_dir = tempdir().unwrap();

    let mut swarm = Swarm::builder()
        .with_directory(temp_dir.path().to_path_buf().try_into().unwrap())
        .with_min_port(10900)
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
    let blockstore = swarm.get_blockstore(&pubkey1).unwrap();

    let file1 = tokio::fs::read("tests/testdir/file1").await.unwrap();
    let file2 = tokio::fs::read("tests/testdir/file2").await.unwrap();
    let file3 = tokio::fs::read("tests/testdir/subdir/file3").await.unwrap();

    let mut writer_sender = blockstore.file_writer().await.unwrap();
    writer_sender.write(&file1, true).await.unwrap();
    let root_hash_file1 = writer_sender.commit().await.unwrap();

    let mut writer_sender = blockstore.file_writer().await.unwrap();
    writer_sender.write(&file2, true).await.unwrap();
    let root_hash_file2 = writer_sender.commit().await.unwrap();

    let mut writer_sender = blockstore.file_writer().await.unwrap();
    writer_sender.write(&file3, true).await.unwrap();
    let root_hash_file3 = writer_sender.commit().await.unwrap();

    let mut writer_sender = blockstore.dir_writer(1).await.unwrap();
    let entry_file3 = BorrowedEntry {
        name: "file3".as_bytes(),
        link: BorrowedLink::Content(&root_hash_file3),
    };
    writer_sender.insert(entry_file3, true).await.unwrap();
    let root_hash_subdir = writer_sender.commit().await.unwrap();

    let mut writer_sender = blockstore.dir_writer(3).await.unwrap();
    let entry_file1 = BorrowedEntry {
        name: "file1".as_bytes(),
        link: BorrowedLink::Content(&root_hash_file1),
    };
    let entry_file2 = BorrowedEntry {
        name: "file2".as_bytes(),
        link: BorrowedLink::Content(&root_hash_file2),
    };
    let entry_subdir = BorrowedEntry {
        name: "subdir".as_bytes(),
        link: BorrowedLink::Content(&root_hash_subdir),
    };

    writer_sender.insert(entry_file1, false).await.unwrap();
    writer_sender.insert(entry_file2, false).await.unwrap();
    writer_sender.insert(entry_subdir, true).await.unwrap();
    let hash = writer_sender.commit().await.unwrap();

    tokio::time::sleep(Duration::from_secs(5)).await;
    // Fetch data from Node 1 to force getting from the other node 0
    let fetcher = swarm.get_fetcher_socket(&pubkey2).unwrap();
    let res = fetcher
        .run(FetcherRequest::Fetch { hash })
        .await
        .expect("Failed to send request");

    tokio::time::sleep(Duration::from_secs(5)).await;
    match res {
        FetcherResponse::Put(_) => panic!("impossible"),
        FetcherResponse::Fetch(hash_new) => {
            if let Err(err) = hash_new {
                panic!("Error {err:?}");
            }
            let blockstore_2 = swarm.get_blockstore(&pubkey2).unwrap();
            let dir_reader = blockstore_2
                .get_bucket()
                .get(&hash)
                .await
                .unwrap()
                .into_dir()
                .unwrap();
            let entry = dir_reader
                .entries()
                .await
                .unwrap()
                .next()
                .await
                .unwrap()
                .unwrap();
            if let OwnedLink::Content(link) = entry.link {
                let mut path_header = blockstore_2.get_root_dir().to_path_buf();
                path_header.push("headers");
                let dir = tokio::fs::read_dir(path_header).await.unwrap();
                assert!(
                    blockstore_2
                        .get_bucket()
                        .get(&link)
                        .await
                        .unwrap()
                        .is_file()
                );
            } else {
                panic!("Error. Expected content");
            }
        },
    }

    swarm.shutdown().await;
}
