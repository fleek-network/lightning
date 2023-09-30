use std::collections::HashMap;

use lightning_interfaces::types::NodeIndex;
use lightning_interfaces::ServiceScope;
use quinn::{ConnectionError, RecvStream, SendStream};
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::{mpsc, oneshot};

pub struct StreamService {
    /// Service handles.
    handles: HashMap<ServiceScope, Sender<(SendStream, RecvStream)>>,
    /// Receive requests for a multiplexed stream.
    stream_request_rx: Receiver<StreamRequest>,
    /// Send handle to return to users as they register with the stream service.
    stream_request_tx: Sender<StreamRequest>,
}

impl StreamService {
    pub fn new() -> Self {
        let (stream_request_tx, stream_request_rx) = mpsc::channel(1024);
        Self {
            handles: HashMap::new(),
            stream_request_tx,
            stream_request_rx,
        }
    }

    pub fn register(
        &mut self,
        service_scope: ServiceScope,
    ) -> (Sender<StreamRequest>, Receiver<(SendStream, RecvStream)>) {
        let (tx, rx) = mpsc::channel(1024);
        self.handles.insert(service_scope, tx);
        (self.stream_request_tx.clone(), rx)
    }

    pub fn handle_incoming_stream(
        &self,
        service_scope: ServiceScope,
        stream: (SendStream, RecvStream),
    ) {
        match self.handles.get(&service_scope).cloned() {
            None => tracing::warn!("received unknown service scope: {service_scope:?}"),
            Some(tx) => {
                tokio::spawn(async move {
                    if tx.send(stream).await.is_err() {
                        tracing::error!("failed to send incoming stream to user");
                    }
                });
            },
        }
    }

    #[inline]
    pub async fn next(&mut self) -> Option<StreamRequest> {
        self.stream_request_rx.recv().await
    }
}

pub struct StreamRequest {
    pub service_scope: ServiceScope,
    pub peer: NodeIndex,
    pub respond: oneshot::Sender<Result<(SendStream, RecvStream), ConnectionError>>,
}
