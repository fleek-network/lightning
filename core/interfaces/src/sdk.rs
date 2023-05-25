use std::{future::Future, pin::Pin};

use async_trait::async_trait;
use tokio::io::{AsyncRead, AsyncWrite};

use crate::{
    application::SyncQueryRunnerInterface, fs::FileSystemInterface, identity::PeerId,
    reputation::ReputationReporterInterface, signer::SubmitTxSocket, types::UpdateMethod,
};

/// The request handler of a service.
pub type HandlerFn<'a, 'b, 'c, S> =
    fn(&'a S, &'b mut <S as SdkInterface>::Connection) -> Pin<Box<dyn Future<Output = ()> + 'c>>;

#[async_trait]
pub trait SdkInterface: Clone + Send + Sync + 'static {
    /// The object that is used to represent a connection in this SDK.
    type Connection: ConnectionInterface;

    /// The type for the sync execution engine.
    type SyncQuery: SyncQueryRunnerInterface;

    /// The reputation reporter used to report measurements about other peers.
    type ReputationReporter: ReputationReporterInterface;

    /// The file system of the SDK.
    type FileSystem: FileSystemInterface;

    /// Returns a new instance of the SDK object.
    fn new(
        q: Self::SyncQuery,
        r: Self::ReputationReporter,
        f: Self::FileSystem,
        t: SubmitTxSocket,
    ) -> Self;

    /// Returns the reputation reporter.
    fn get_reputation_reporter(&self) -> &Self::ReputationReporter;

    /// Returns the sync query runner.
    fn get_sync_query(&self) -> &Self::SyncQuery;

    /// Returns the file system.
    fn get_fs(&self) -> &Self::FileSystem;

    /// Submit a transaction by the current node.
    fn submit_transaction(&self, tx: UpdateMethod);
}

pub trait ConnectionInterface: Send + Sync {
    /// The writer half of this connection.
    type Writer: AsyncWrite;

    /// The reader half of this connection.
    type Reader: AsyncRead;

    /// Split the connection to the `writer` and `reader` half and returns a mutable reference to
    /// both sides.
    fn split(&mut self) -> (&mut Self::Writer, &mut Self::Reader);

    /// Returns a mutable reference to the writer half of this connection.
    fn writer(&mut self) -> &mut Self::Writer;

    /// Returns a mutable reference to the reader half of this connection.
    fn reader(&mut self) -> &mut Self::Reader;

    /// Returns the lane number associated with this connection.
    fn get_lane(&self) -> u8;

    /// Returns the ID of the client that has established this connection.
    fn get_client(&self) -> &PeerId;
}
