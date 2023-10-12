use std::path::PathBuf;
use std::time::Duration;

use ethers::types::{BlockNumber, U64};
use fleek_crypto::{NodeSecretKey, SecretKey, TransactionSender, TransactionSignature};
use lightning_application::app::Application;
use lightning_application::config::Config as AppConfig;
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::types::{
    ArchiveRequest,
    Block,
    BlockExecutionResponse,
    ExecutionData,
    IndexRequest,
    TransactionReceipt,
    TransactionRequest,
    TransactionResponse,
    UpdateMethod,
    UpdatePayload,
    UpdateRequest,
};
use lightning_interfaces::{
    partial,
    ApplicationInterface,
    ArchiveInterface,
    ToDigest,
    WithStartAndShutdown,
};

use crate::archive::Archive;
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

fn get_transactions(num_txns: usize) -> Vec<UpdateRequest> {
    (0..num_txns)
        .map(|i| {
            let secret_key = NodeSecretKey::generate();
            let public_key = secret_key.to_pk();
            let sender = TransactionSender::NodeMain(public_key);
            let method = UpdateMethod::ChangeEpoch { epoch: i as u64 };
            let payload = UpdatePayload { nonce: 0, method };
            let digest = payload.to_digest();
            UpdateRequest {
                sender,
                signature: TransactionSignature::NodeMain(secret_key.sign(&digest)),
                payload,
            }
        })
        .collect()
}

fn get_index_request(block_index: u8, parent_hash: [u8; 32]) -> IndexRequest {
    let transactions = get_transactions(5);
    let digest = [block_index; 32];

    let txn_receipts: Vec<TransactionReceipt> = transactions
        .iter()
        .enumerate()
        .map(|(i, tx)| {
            TransactionReceipt {
                block_hash: digest,
                block_number: block_index as u128,
                transaction_index: i as u64,
                transaction_hash: tx.payload.to_digest(), // dummy hash
                from: tx.sender,
                response: TransactionResponse::Success(ExecutionData::None),
            }
        })
        .collect();

    let block = Block {
        transactions: transactions
            .into_iter()
            .map(TransactionRequest::UpdateRequest)
            .collect(),
        digest,
    };

    let receipt = BlockExecutionResponse {
        block_number: block_index as u128,
        block_hash: digest,
        parent_hash,
        change_epoch: false,
        node_registry_delta: vec![],
        txn_receipts,
    };

    IndexRequest { block, receipt }
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
