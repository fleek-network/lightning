use lightning_application::app::Application;
use lightning_application::config::Config as AppConfig;
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::{
    partial,
    ApplicationInterface,
    ConsensusInterface,
    PoolInterface,
    SignerInterface,
    WithStartAndShutdown,
};
use lightning_signer::{Config as SignerConfig, Signer};
use lightning_test_utils::consensus::{Config as ConsensusConfig, MockConsensus};

use crate::config::Config;
use crate::pool::Pool;

partial!(TestBinding {
    SignerInterface = Signer<Self>;
    ApplicationInterface = Application<Self>;
    ConsensusInterface = MockConsensus<Self>;
});

#[tokio::test]
async fn test_shutdown_and_start_again() {
    let app =
        Application::<TestBinding>::init(AppConfig::test(), Default::default(), Default::default())
            .unwrap();
    let (update_socket, query_runner) = (app.transaction_executor(), app.sync_query());
    let mut signer =
        Signer::<TestBinding>::init(SignerConfig::test(), query_runner.clone()).unwrap();
    let consensus = MockConsensus::<TestBinding>::init(
        ConsensusConfig::default(),
        &signer,
        update_socket.clone(),
        query_runner.clone(),
        infusion::Blank::default(),
    )
    .unwrap();
    signer.provide_mempool(consensus.mempool());
    signer.provide_new_block_notify(consensus.new_block_notifier());

    let pool = Pool::<TestBinding>::init(Config::default(), &signer).unwrap();

    assert!(!pool.is_running());
    pool.start().await;
    assert!(pool.is_running());
    pool.shutdown().await;
    assert!(!pool.is_running());

    pool.start().await;
    assert!(pool.is_running());
    pool.shutdown().await;
    assert!(!pool.is_running());
}
