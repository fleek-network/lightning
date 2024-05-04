use std::time::Duration;

use ethers::types::BlockNumber;
use lightning_application::app::Application;
use lightning_application::config::Config as AppConfig;
use lightning_blockstore::blockstore::Blockstore;
use lightning_blockstore::config::Config as BlockstoreConfig;
use lightning_interfaces::prelude::*;
use lightning_interfaces::{partial, Ref};
use lightning_notifier::Notifier;
use lightning_test_utils::consensus::{
    Config as MockConsensusConfig,
    MockConsensus,
    MockForwarder,
};
use lightning_test_utils::defer::Deferred;
use lightning_test_utils::json_config::JsonConfigProvider;
use lightning_test_utils::transaction::get_update_transactions;
use resolved_pathbuf::ResolvedPathBuf;

use crate::archive::Archive;
use crate::config::Config as ArchiveConfig;

partial!(TestBinding {
    ApplicationInterface = Application<Self>;
    ArchiveInterface = Archive<Self>;
    BlockstoreInterface = Blockstore<Self>;
    NotifierInterface = Notifier<Self>;
    ConsensusInterface = MockConsensus<Self>;
    ForwarderInterface = MockForwarder<Self>;
    ConfigProviderInterface = JsonConfigProvider;
});

async fn get_node() -> Node<TestBinding> {
    let path = std::env::temp_dir().join(std::thread::current().name().unwrap());

    if path.exists() {
        std::fs::remove_dir_all(&path).unwrap();
    }

    let path = ResolvedPathBuf::try_from(path).unwrap();

    let node = Node::<TestBinding>::init(
        JsonConfigProvider::default()
            .with::<Application<TestBinding>>(AppConfig::test())
            .with::<Archive<TestBinding>>(ArchiveConfig {
                is_archive: true,
                store_path: {
                    let mut p = path.clone();
                    p.push("archive");
                    p
                },
            })
            .with::<Blockstore<TestBinding>>(BlockstoreConfig {
                root: {
                    let mut p = path.clone();
                    p.push("store");
                    p
                },
            })
            .with::<MockConsensus<TestBinding>>(MockConsensusConfig {
                min_ordering_time: 0,
                max_ordering_time: 0,
                probability_txn_lost: 0.0,
                transactions_to_lose: Default::default(),
                new_block_interval: Duration::from_secs(0),
            }),
    )
    .unwrap();

    node.provider.insert(Deferred::remove_dir_all(path));

    node.start().await;

    node
}

#[tokio::test]
async fn test_archive_api() {
    const NUM_TX: usize = 3;

    let mut node = get_node().await;

    let archive: Ref<Archive<TestBinding>> = node.provider.get();

    let notifier: Ref<Notifier<TestBinding>> = node.provider.get();
    let mut sub = notifier.subscribe_block_executed();

    let forwarder: Ref<MockForwarder<TestBinding>> = node.provider.get();
    let socket = forwarder.mempool_socket();
    let transactions = get_update_transactions(NUM_TX)
        .into_iter()
        .map(types::TransactionRequest::UpdateRequest)
        .collect::<Vec<_>>();

    // Before sending any transactions we should return None on get tx

    // test earliest and latest
    assert_eq!(
        archive
            .get_block_by_number(BlockNumber::Earliest)
            .await
            .as_ref(),
        None
    );
    assert_eq!(
        archive
            .get_block_by_number(BlockNumber::Latest)
            .await
            .as_ref(),
        None
    );

    // Run the transactions.

    for tx in &transactions {
        socket.run(tx.clone()).await.unwrap();
    }

    let mut block_receipts = Vec::new();

    for tx in &transactions {
        // Wait until the transaction is executed.
        let n = sub.recv().await.unwrap();
        let block_hash = n.response.block_hash;
        let block_num = n.response.block_number;

        let (block_receipt, mut tx_receipts) = n.response.to_receipts();
        // assumption about how MockConsensus works: each transaction is sent as a block.
        assert_eq!(tx_receipts.len(), 1);
        let tx_receipt = tx_receipts.pop().unwrap();

        assert_eq!(
            archive.get_block_by_hash(block_hash).await.as_ref(),
            Some(&block_receipt)
        );

        assert_eq!(
            archive.get_block_by_number(block_num.into()).await.as_ref(),
            Some(&block_receipt)
        );

        assert_eq!(
            archive
                .get_transaction_receipt(tx_receipt.transaction_hash)
                .await
                .as_ref(),
            Some(&tx_receipt)
        );

        assert_eq!(
            archive
                .get_transaction(tx_receipt.transaction_hash)
                .await
                .as_ref(),
            Some(tx)
        );

        block_receipts.push(block_receipt);
    }

    // test pending
    assert_eq!(
        archive
            .get_block_by_number(BlockNumber::Pending)
            .await
            .as_ref(),
        None
    );

    // test earliest
    assert_eq!(
        archive
            .get_block_by_number(BlockNumber::Earliest)
            .await
            .as_ref(),
        Some(&block_receipts[0])
    );

    // test latest
    assert_eq!(
        archive
            .get_block_by_number(BlockNumber::Latest)
            .await
            .as_ref(),
        Some(&block_receipts[block_receipts.len() - 1])
    );

    node.shutdown().await;
}
