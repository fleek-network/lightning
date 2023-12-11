use std::sync::Arc;

use lightning_types::Event;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::broadcast;
use tokio::sync::oneshot;
use tokio::sync::{mpsc, Mutex};

pub struct EventDistributor {
    listeners: Arc<Mutex<usize>>,

    /// A sender that receives events from the application layer
    event_tx: mpsc::Sender<Event>,

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

        let listeners = Arc::new(Mutex::new(0));

        let distributor = Self {
            listeners: listeners.clone(),
            event_tx: event_tx.clone(),
            broadcast_tx: broadcast_tx.clone(),
            shutdown: shutdown_tx,
        };

        let fut = async move {
            // Recieve events from the application layer
            while let Some(event) = event_rx.recv().await {
                // If there are no listeners, don't bother broadcasting anything
                if *listeners.lock().await > 0 {
                    match broadcast_tx.send(event) {
                        Ok(_) => {}
                        Err(_) => {
                            tracing::error!("The event broadcast channel failed to send and eveent")
                        }
                    }
                }
            }
        };

        tokio::spawn(async {
            tokio::select! {
                _ = fut => {
                    tracing::error!("Event distributor future returned; this is a bug")
                }
                _ = shutdown_rx => {
                    tracing::trace!("Event recieved shutdown signal")
                }
            }
        });

        distributor
    }

    pub fn sender(&self) -> mpsc::Sender<Event> {
        self.event_tx.clone()
    }

    pub fn register_listener(&self) -> broadcast::Receiver<Event> {
        let listeners = self.listeners.clone();

        tokio::spawn(async move {
            let mut listeners = listeners.lock().await;
            *listeners += 1;
        });

        self.broadcast_tx.subscribe()
    }

    pub async fn shutdown(self) {
        let _ = self.shutdown.send(());
    }
}

/// This is a wrapper over a broadcast receiver that can be used in exactly the same way
pub struct EventReceiver<'a> {
    distributor: &'a EventDistributor,
    receiver: broadcast::Receiver<Event>,
}

impl<'a> EventReceiver<'a> {
    pub fn new(distributor: &'a EventDistributor) -> Self {
        let receiver = distributor.register_listener();
        Self {
            distributor,
            receiver,
        }
    }

    pub async fn recv(&mut self) -> Result<Event, RecvError> {
        self.receiver.recv().await
    }
}

impl<'a> Drop for EventReceiver<'a> {
    fn drop(&mut self) {
        let listeners = self.distributor.listeners.clone();

        tokio::spawn(async move {
            let mut listeners = listeners.lock().await;
            *listeners -= 1;
        });
    }
}
