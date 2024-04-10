use lightning_interfaces::types::Event;
use tokio::sync::{broadcast, mpsc, oneshot};

pub struct EventDistributor {
    /// A sender that receives events from the application layer
    event_tx: mpsc::Sender<Vec<Event>>,

    /// A clonable reciever that can be used to register listeners
    broadcast_tx: broadcast::Sender<Event>,

    /// Shutdown signal
    shutdown: oneshot::Sender<()>,
}

impl EventDistributor {
    pub fn spawn() -> Self {
        let (event_tx, mut event_rx) = mpsc::channel(100);
        let (broadcast_tx, _) = broadcast::channel(100);
        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();

        let this = Self {
            event_tx,
            broadcast_tx: broadcast_tx.clone(),
            shutdown: shutdown_tx,
        };

        tokio::spawn(async move {
            tokio::select! {
                _ = Self::forward(&mut event_rx, &broadcast_tx) => {
                    tracing::info!("consensous dropped the tx")
                }
                _ = shutdown_rx => {
                    tracing::trace!("Event recieved shutdown signal")
                }
            }
        });

        this
    }

    async fn forward(rx: &mut mpsc::Receiver<Vec<Event>>, tx: &broadcast::Sender<Event>) {
        while let Some(events) = rx.recv().await {
            if tx.receiver_count() > 0 {
                for event in events {
                    match tx.send(event) {
                        Ok(_) => {},
                        Err(e) => {
                            tracing::error!(
                                "The event broadcast channel failed to send and event {:?}",
                                e
                            )
                        },
                    }
                }
            }
        }
    }

    pub async fn shutdown(self) {
        let _ = self.shutdown.send(());
    }
}

impl EventDistributor {
    pub fn sender(&self) -> mpsc::Sender<Vec<Event>> {
        self.event_tx.clone()
    }

    pub fn register_listener(&self) -> broadcast::Receiver<Event> {
        self.broadcast_tx.subscribe()
    }
}
