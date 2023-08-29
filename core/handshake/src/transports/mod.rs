use async_trait::async_trait;
use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::schema;

pub mod mock;
pub mod webrtc;

#[async_trait]
pub trait Transport: Sized {
    type Config: Default + Serialize + DeserializeOwned;

    type Connection: Connection;

    /// Bind the transport with the provided config.
    async fn bind(config: Self::Config) -> anyhow::Result<Self>;

    /// Accept a new connection.
    async fn accept(&mut self) -> Option<(schema::HandshakeRequestFrame, Self::Connection)>;
}

#[async_trait]
pub trait Connection: Sized + Send + Sync + 'static {
    /// Send the initial handshake response to the client.
    fn send_handshake_response(&mut self, response: schema::HandshakeResponse);

    /// Send a frame to the client.
    fn send(&mut self, frame: schema::ResponseFrame);

    /// Receive a frame from the connection. Returns `None` when the connection
    /// is closed.
    async fn recv(&mut self) -> Option<schema::RequestFrame>;

    /// Terminate the connection
    fn terminate(mut self, reason: schema::TerminationReason) {
        self.send(schema::ResponseFrame::Termination { reason })
    }
}
