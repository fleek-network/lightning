use std::{
    pin::Pin,
    task::{Context, Poll},
};

use async_trait::async_trait;
use draco_interfaces::{
    config::ConfigConsumer, signer::SubmitTxSocket, types::UpdateMethod, ConnectionInterface,
    SdkInterface,
};
use fleek_crypto::ClientPublicKey;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf, Result};

use super::{
    application::QueryRunner, config::Config, fs::FileSystem, reputation::MyReputationReporter,
};

#[derive(Clone)]
pub struct Sdk {}

#[async_trait]
impl SdkInterface for Sdk {
    /// The object that is used to represent a connection in this SDK.
    type Connection = MyConnection;

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

impl ConfigConsumer for Sdk {
    const KEY: &'static str = "sdk";

    type Config = Config;
}

pub struct MyConnection {}

impl ConnectionInterface for MyConnection {
    /// The writer half of this connection.
    type Writer = MyWriter;

    /// The reader half of this connection.
    type Reader = MyReader;

    /// Split the connection to the `writer` and `reader` half and returns a mutable reference to
    /// both sides.
    fn split(&mut self) -> (&mut Self::Writer, &mut Self::Reader) {
        todo!()
    }

    /// Returns a mutable reference to the writer half of this connection.
    fn writer(&mut self) -> &mut Self::Writer {
        todo!()
    }

    /// Returns a mutable reference to the reader half of this connection.
    fn reader(&mut self) -> &mut Self::Reader {
        todo!()
    }

    /// Returns the lane number associated with this connection.
    fn get_lane(&self) -> u8 {
        todo!()
    }

    /// Returns the ID of the client that has established this connection.
    fn get_client(&self) -> &ClientPublicKey {
        todo!()
    }
}

pub struct MyWriter {}

impl AsyncWrite for MyWriter {
    fn poll_write(self: Pin<&mut Self>, _cx: &mut Context<'_>, _buf: &[u8]) -> Poll<Result<usize>> {
        // Your implementation for writing to the underlying resource goes here
        // You can use `buf` to access the bytes to write
        // Return `Poll::Ready(Ok(bytes_written))` when writing is complete

        Poll::Ready(Ok(0)) // Dummy implementation, always returns 0 bytes written
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<()>> {
        // Your implementation for flushing any buffered data goes here
        // Return `Poll::Ready(Ok(()))` when flushing is complete

        Poll::Ready(Ok(())) // Dummy implementation, always returns success
    }

    fn poll_shutdown(self: Pin<&mut Self>, _: &mut std::task::Context<'_>) -> Poll<Result<()>> {
        todo!()
    }
}

pub struct MyReader {}

impl AsyncRead for MyReader {
    fn poll_read(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        _buf: &mut ReadBuf<'_>,
    ) -> Poll<Result<()>> {
        // Your implementation for reading from the underlying resource goes here
        // Write the read bytes to the `buf` slice
        // Return `Poll::Ready(Ok(bytes_read))` when reading is complete

        Poll::Ready(Ok(())) // Dummy implementation, always returns 0 bytes read
    }
}
