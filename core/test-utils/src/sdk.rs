//! This file should be updated as each crate gets implemented
//! TODO: move into test-utils

use std::marker::PhantomData;

use async_trait::async_trait;
use draco_application::query_runner::QueryRunner;
use draco_handshake::server::RawLaneConnection;
use draco_interfaces::{
    config::ConfigConsumer, signer::SubmitTxSocket, types::UpdateMethod, SdkInterface,
};
use tokio::io::{AsyncRead, AsyncWrite};

use crate::{
    empty_interfaces::{MockConfig, MockReputationReporter},
    filesystem::MockFileSystem,
};

pub struct MockSdk<R, W> {
    phantom: PhantomData<(R, W)>,
    query: QueryRunner,
    rep_reporter: MockReputationReporter,
    fs: MockFileSystem,
    tx: SubmitTxSocket,
}

impl<R, W> Clone for MockSdk<R, W> {
    fn clone(&self) -> Self {
        Self {
            phantom: self.phantom,
            query: self.query.clone(),
            rep_reporter: self.rep_reporter.clone(),
            fs: self.fs.clone(),
            tx: self.tx.clone(),
        }
    }
}

#[async_trait]
impl<R: AsyncRead + Unpin + Send + Sync + 'static, W: AsyncWrite + Unpin + Send + Sync + 'static>
    SdkInterface for MockSdk<R, W>
{
    /// The object that is used to represent a connection in this SDK.
    type Connection = RawLaneConnection<R, W>;

    /// The type for the sync execution engine.
    type SyncQuery = QueryRunner;

    /// The reputation reporter used to report measurements about other peers.
    type ReputationReporter = MockReputationReporter;

    /// The file system of the SDK.
    type FileSystem = MockFileSystem;

    /// Returns a new instance of the SDK object.
    fn new(
        query: Self::SyncQuery,
        rep_reporter: Self::ReputationReporter,
        fs: Self::FileSystem,
        tx: SubmitTxSocket,
    ) -> Self {
        Self {
            phantom: PhantomData::<(R, W)>,
            query,
            rep_reporter,
            fs,
            tx,
        }
    }

    /// Returns the reputation reporter.
    fn get_reputation_reporter(&self) -> &Self::ReputationReporter {
        &self.rep_reporter
    }

    /// Returns the sync query runner.
    fn get_sync_query(&self) -> &Self::SyncQuery {
        &self.query
    }

    /// Returns the file system.
    fn get_fs(&self) -> &Self::FileSystem {
        &self.fs
    }

    /// Submit a transaction by the current node.
    fn submit_transaction(&self, _tx: UpdateMethod) {}
}

impl<R, W> ConfigConsumer for MockSdk<R, W> {
    const KEY: &'static str = "sdk";

    type Config = MockConfig;
}
