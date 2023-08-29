use std::collections::HashMap;

use fleek_crypto::NodePublicKey;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::oneshot;

type Message = Vec<u8>;

pub enum Request {
    SendMessage {
        /// Peer to connect to.
        peer: NodePublicKey,
        /// Respond with connection information such as
        /// negotiated parameters during handshake and peer IP.
        conn_info_tx: Sender<()>,
    },
    Metrics {
        /// Connection peer.
        peer: NodePublicKey,
        /// Channel to respond on with metrics.
        respond: oneshot::Sender<()>,
    },
}

pub struct Endpoint {
    /// Used for sending outbound messages to drivers.
    driver: HashMap<NodePublicKey, Sender<Message>>,
    /// Pending outgoing messages.
    pending_send: HashMap<NodePublicKey, Message>,
    /// Input requests for the endpoint.
    request_rx: Receiver<Request>,
}
