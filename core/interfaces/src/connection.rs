use lightning_types::ConnectionMetadata;

/// The connection type that is offered by the (HandshakeInterface)[crate::HandshakeInterface].
///
/// This is a connection provided by the handshake to the service executor, a connection is always
/// under a service. Once a service is done with the connection they will naturally drop the
/// connection that is when the custom implementation of this connection object should put the
/// connection back to the handshake so that the user is able to reuse the connection for the next
/// request they may have.
#[infusion::blank]
pub trait ConnectionInterface: Send + Sync {
    fn get_metadata(&self) -> ConnectionMetadata;

    // TODO: Reader and writer.
    // Assume we have a framed reader/write with length-prefixed bytes over
    // the stream.
}
