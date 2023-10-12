use std::time::Duration;

use lightning_application::app::Application;
use lightning_application::config::Config as AppConfig;
use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::{partial, ApplicationInterface, ArchiveInterface, WithStartAndShutdown};

use crate::archive::Archive;
use crate::config::Config;

partial!(TestBinding {
    ApplicationInterface = Application<Self>;
    ArchiveInterface = Archive<Self>;
});

#[tokio::test]
async fn test_shutdown_and_start_again() {
    let app = Application::<TestBinding>::init(AppConfig::test(), Default::default()).unwrap();
    let (_, query_runner) = (app.transaction_executor(), app.sync_query());

    let path = std::env::temp_dir().join("lightning-test-archive-start-shutdown");
    let archive = Archive::<TestBinding>::init(
        Config {
            is_archive: true,
            store_path: Some(path.clone().try_into().unwrap()),
        },
        query_runner,
    )
    .unwrap();

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
