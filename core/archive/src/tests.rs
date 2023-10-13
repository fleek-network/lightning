use std::path::PathBuf;
use std::time::Duration;

use ethers::types::{BlockNumber, U64};
use lightning_application::app::Application;
use lightning_application::config::Config as AppConfig;
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::types::{ArchiveRequest, ArchiveResponse};
use lightning_interfaces::{partial, ApplicationInterface, ArchiveInterface, WithStartAndShutdown};
use lightning_test_utils::transaction::get_index_request;

use crate::archive::{Archive, BlockInfo};
use crate::config::Config;

partial!(TestBinding {
    ApplicationInterface = Application<Self>;
    ArchiveInterface = Archive<Self>;
});

async fn init_archive(path: &str) -> (Archive<TestBinding>, Application<TestBinding>, PathBuf) {
    let app = Application::<TestBinding>::init(AppConfig::test(), Default::default()).unwrap();
    let (_, query_runner) = (app.transaction_executor(), app.sync_query());
    app.start().await;

    let path = std::env::temp_dir().join(path);

    if path.exists() {
        std::fs::remove_dir_all(&path).unwrap();
    }

    let archive = Archive::<TestBinding>::init(
        Config {
            is_archive: true,
            store_path: path.clone().try_into().unwrap(),
        },
        query_runner,
    )
    .unwrap();
    (archive, app, path)
}

#[tokio::test]
async fn test_shutdown_and_start_again() {
    let (archive, _app, path) = init_archive("lightning-test-archive-start-shutdown").await;

    assert!(!archive.is_running());
    archive.start().await;
    assert!(archive.is_running());
    archive.shutdown().await;
    // Since shutdown is no longer doing async operations we need to wait a millisecond for it to
    // finish shutting down
    tokio::time::sleep(Duration::from_millis(1)).await;
    assert!(!archive.is_running());

    archive.start().await;
    assert!(archive.is_running());
    archive.shutdown().await;
    tokio::time::sleep(Duration::from_millis(1)).await;
    assert!(!archive.is_running());

    if path.exists() {
        std::fs::remove_dir_all(path).unwrap();
    }
}

#[tokio::test]
async fn test_get_block_by_num_and_hash() {
    let (archive, _app, path) = init_archive("lightning-test-get-block-by-num-and-hash").await;
    let index_socket = archive.index_socket().unwrap();
    let archive_socket = archive.archive_socket().unwrap();
    archive.start().await;

    let index_req = get_index_request(0, [0; 32]);
    index_socket.run(index_req.clone()).await.unwrap().unwrap();

    let block1 = archive_socket
        .run(ArchiveRequest::GetBlockByHash(index_req.receipt.block_hash))
        .await
        .unwrap()
        .unwrap();

    let block2 = archive_socket
        .run(ArchiveRequest::GetBlockByNumber(BlockNumber::Number(
            U64::from(index_req.receipt.block_number as u64),
        )))
        .await
        .unwrap()
        .unwrap();

    assert_eq!(block1, block2);

    if path.exists() {
        std::fs::remove_dir_all(path).unwrap();
    }
}

#[tokio::test]
async fn test_get_tx() {
    let (archive, _app, path) = init_archive("lightning-test-get-tx").await;
    let index_socket = archive.index_socket().unwrap();
    let archive_socket = archive.archive_socket().unwrap();
    archive.start().await;

    let index_req = get_index_request(1, [1; 32]);

    let target_tx = index_req.block.transactions[0].clone();
    let tx_receipt = index_req.receipt.txn_receipts[0].clone();

    index_socket.run(index_req).await.unwrap().unwrap();

    let tx = archive_socket
        .run(ArchiveRequest::GetTransaction(tx_receipt.transaction_hash))
        .await
        .unwrap()
        .unwrap();

    match tx {
        ArchiveResponse::Transaction(tx) => assert_eq!(tx, target_tx),
        _ => panic!("Unexpected response"),
    }

    if path.exists() {
        std::fs::remove_dir_all(path).unwrap();
    }
}

#[tokio::test]
async fn test_get_tx_receipt() {
    let (archive, _app, path) = init_archive("lightning-test-get-tx-receipt").await;
    let index_socket = archive.index_socket().unwrap();
    let archive_socket = archive.archive_socket().unwrap();
    archive.start().await;

    let index_req = get_index_request(1, [1; 32]);

    let target_tx_receipt = index_req.receipt.txn_receipts[0].clone();

    index_socket.run(index_req).await.unwrap().unwrap();

    let tx_receipt = archive_socket
        .run(ArchiveRequest::GetTransactionReceipt(
            target_tx_receipt.transaction_hash,
        ))
        .await
        .unwrap()
        .unwrap();

    match tx_receipt {
        ArchiveResponse::TransactionReceipt(tx_receipt) => {
            assert_eq!(tx_receipt, target_tx_receipt)
        },
        _ => panic!("Unexpected response"),
    }

    if path.exists() {
        std::fs::remove_dir_all(path).unwrap();
    }
}

#[tokio::test]
async fn test_get_latest_earliest() {
    let (archive, _app, path) = init_archive("lightning-test-get-latest-earliest").await;
    let index_socket = archive.index_socket().unwrap();
    let archive_socket = archive.archive_socket().unwrap();
    archive.start().await;

    let index_req1 = get_index_request(0, [0; 32]);
    index_socket.run(index_req1.clone()).await.unwrap().unwrap();

    let block = archive_socket
        .run(ArchiveRequest::GetBlockByNumber(BlockNumber::Latest))
        .await
        .unwrap()
        .unwrap();

    match block {
        ArchiveResponse::Block(block) => {
            assert_eq!(block.block_hash, index_req1.receipt.block_hash);
        },
        _ => panic!("Unexpected response"),
    }

    let index_req2 = get_index_request(1, [1; 32]);
    index_socket.run(index_req2.clone()).await.unwrap().unwrap();

    let block = archive_socket
        .run(ArchiveRequest::GetBlockByNumber(BlockNumber::Latest))
        .await
        .unwrap()
        .unwrap();

    match block {
        ArchiveResponse::Block(block) => {
            assert_eq!(block.block_hash, index_req2.receipt.block_hash);
        },
        _ => panic!("Unexpected response"),
    }

    let block = archive_socket
        .run(ArchiveRequest::GetBlockByNumber(BlockNumber::Earliest))
        .await
        .unwrap()
        .unwrap();

    match block {
        ArchiveResponse::Block(block) => {
            assert_eq!(block.block_hash, index_req1.receipt.block_hash);
        },
        _ => panic!("Unexpected response"),
    }

    if path.exists() {
        std::fs::remove_dir_all(path).unwrap();
    }
}

#[tokio::test]
async fn test_get_pending() {
    let (archive, _app, path) = init_archive("lightning-test-get-pending").await;
    let index_socket = archive.index_socket().unwrap();
    let archive_socket = archive.archive_socket().unwrap();
    archive.start().await;

    let index_req = get_index_request(0, [0; 32]);
    index_socket.run(index_req.clone()).await.unwrap().unwrap();

    let block = archive_socket
        .run(ArchiveRequest::GetBlockByNumber(BlockNumber::Pending))
        .await
        .unwrap()
        .unwrap();

    assert_eq!(block, ArchiveResponse::None);

    if path.exists() {
        std::fs::remove_dir_all(path).unwrap();
    }
}

#[test]
fn test_block_info_ser_deser() {
    let index_request = get_index_request(0, [0; 32]);
    let (blk_receipt, _txn_receipts) = index_request.receipt.to_receipts();
    let blk_info = BlockInfo {
        block: index_request.block,
        receipt: blk_receipt,
    };
    let bytes: Vec<u8> = (&blk_info).try_into().unwrap();
    let blk_info_r = BlockInfo::try_from(bytes).unwrap();
    assert_eq!(blk_info, blk_info_r);
}
