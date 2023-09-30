use std::collections::HashMap;

use lightning_interfaces::ServiceScope;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::{mpsc, oneshot};

pub struct StreamService<W, R> {
    /// Service handles.
    handles: HashMap<ServiceScope, Sender<(W, R)>>,
    /// Receive requests for a multiplexed stream.
    stream_rx: Receiver<StreamRequest<W, R>>,
}

impl<W, R> StreamService<W, R>
where
    W: AsyncWrite + Send + 'static,
    R: AsyncRead + Send + 'static,
{
    pub fn new(stream_rx: Receiver<StreamRequest<W, R>>) -> Self {
        Self {
            handles: HashMap::new(),
            stream_rx,
        }
    }

    pub fn register(&mut self, service_scope: ServiceScope) -> Receiver<(W, R)> {
        let (tx, rx) = mpsc::channel(1024);
        self.handles.insert(service_scope, tx);
        rx
    }

    #[allow(unused)]
    pub fn handle_incoming_stream(&self, service_scope: ServiceScope, stream: (W, R)) {
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

    pub async fn next(&mut self) -> Option<oneshot::Sender<(W, R)>> {
        let StreamRequest { respond } = self.stream_rx.recv().await?;
        Some(respond)
    }
}

pub struct StreamRequest<W, R> {
    pub respond: oneshot::Sender<(W, R)>,
}
