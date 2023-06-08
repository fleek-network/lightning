use std::marker::PhantomData;

use async_trait::async_trait;
use draco_application::query_runner::QueryRunner;
use draco_handshake::server::RawLaneConnection;
use draco_interfaces::{
    config::ConfigConsumer, signer::SubmitTxSocket, types::UpdateMethod, SdkInterface,
};
use draco_rep_collector::MyReputationReporter;
use tokio::io::{AsyncRead, AsyncWrite};

use super::{config::Config, fs::FileSystem};

pub struct Sdk<R: AsyncRead + Unpin + Send + Sync, W: AsyncWrite + Unpin + Send + Sync> {
    phantom: PhantomData<(R, W)>,
}

impl<R: AsyncRead + Unpin + Send + Sync, W: AsyncWrite + Unpin + Send + Sync> Clone for Sdk<R, W> {
    fn clone(&self) -> Self {
        Self {
            phantom: self.phantom,
        }
    }
}

#[async_trait]
impl<R: AsyncRead + Unpin + Send + Sync + 'static, W: AsyncWrite + Unpin + Send + Sync + 'static>
    SdkInterface for Sdk<R, W>
{
    /// The object that is used to represent a connection in this SDK.
    type Connection = RawLaneConnection<R, W>;

    /// The type for the sync execution engine.
    type SyncQuery = QueryRunner;

    /// The reputation reporter used to report measurements about other peers.
    type ReputationReporter = MyReputationReporter;

    /// The file system of the SDK.
    type FileSystem = FileSystem;

    /// Returns a new instance of the SDK object.
    fn new(
        _q: Self::SyncQuery,
        _r: Self::ReputationReporter,
        _f: Self::FileSystem,
        _t: SubmitTxSocket,
    ) -> Self {
        todo!()
    }

    /// Returns the reputation reporter.
    fn get_reputation_reporter(&self) -> &Self::ReputationReporter {
        todo!()
    }

    /// Returns the sync query runner.
    fn get_sync_query(&self) -> &Self::SyncQuery {
        todo!()
    }

    /// Returns the file system.
    fn get_fs(&self) -> &Self::FileSystem {
        todo!()
    }

    /// Submit a transaction by the current node.
    fn submit_transaction(&self, _tx: UpdateMethod) {
        todo!()
    }
}

impl<R: AsyncRead + Unpin + Send + Sync, W: AsyncWrite + Unpin + Send + Sync> ConfigConsumer
    for Sdk<R, W>
{
    const KEY: &'static str = "sdk";

    type Config = Config;
}
