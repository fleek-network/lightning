use std::sync::Arc;

use lightning_types::Event;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::{broadcast, mpsc, oneshot, Mutex, Notify};

pub struct EventDistributor {
    /// An internal counter of how many people are using the broadcast rx
    listeners: Arc<Mutex<usize>>,

    /// Rather than lock the listners mutex in a loop, we can use this to notify us when listners
    /// is updated from zero
    start_broadcasting: Arc<Notify>,

    /// Rather than lock the listeners mutex in a loop, we can use this to notify us when listners
    /// becomes zero
    stop_broadcasting: Arc<Notify>,

    /// A sender that receives events from the application layer
    event_tx: mpsc::Sender<Vec<Event>>,

    /// A clonable reciever that can be used to register listeners
    broadcast_tx: broadcast::Sender<Event>,

    /// Shutdown signal
    shutdown: oneshot::Sender<()>,
}

impl EventDistributor {
    pub fn spawn() -> Self {
        let (event_tx, event_rx) = mpsc::channel(100);
        let (broadcast_tx, _) = broadcast::channel(100);
        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
        let start_broadcasting = Arc::new(Notify::new());
        let stop_broadcasting = Arc::new(Notify::new());

        let listeners = Arc::new(Mutex::new(0));

        let this = Self {
            listeners: listeners.clone(),
            event_tx: event_tx.clone(),
            broadcast_tx: broadcast_tx.clone(),
            shutdown: shutdown_tx,
            start_broadcasting: start_broadcasting.clone(),
            stop_broadcasting: stop_broadcasting.clone(),
        };

        let fut = async move {
            loop {
                start_broadcasting.notified().await;

                tokio::select! {
                    _ = Self::forward(event_rx, broadcast_tx) => {
                        break;
                    },
                    _ = stop_broadcasting.notified() => {
                        break;
                    }
                }
            }
        };

        tokio::spawn(async {
            tokio::select! {
                _ = fut => {
                    tracing::error!("Event distrubutor future returned; this is a bug")
                }
                _ = shutdown_rx => {
                    tracing::trace!("Event recieved shutdown signal")
                }
            }
        });

        this
    }

    async fn forward(mut rx: mpsc::Receiver<Vec<Event>>, tx: broadcast::Sender<Event>) {
        while let Some(events) = rx.recv().await {
            for event in events {
                match tx.send(event) {
                    Ok(_) => {},
                    Err(_) => {
                        tracing::error!("The event broadcast channel failed to send and eveent")
                    },
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

    pub fn register_listener(&self) -> EventReceiver<'_> {
        let listeners = self.listeners.clone();
        let start_broadcasting = self.start_broadcasting.clone();

        let rx = self.broadcast_tx.subscribe();

        tokio::spawn(async move {
            let mut listeners = listeners.lock().await;
            *listeners += 1;

            if *listeners == 1 {
                start_broadcasting.notify_one();
            }
        });

        EventReceiver::new(self, rx)
    }
}

/// This is a wrapper over a tokio broadcast receiver that can be used in exactly the same way
pub struct EventReceiver<'a> {
    distributor: &'a EventDistributor,
    rx: broadcast::Receiver<Event>,
}

impl<'a> EventReceiver<'a> {
    pub fn new(distributor: &'a EventDistributor, rx: broadcast::Receiver<Event>) -> Self {
        Self { distributor, rx }
    }

    pub async fn recv(&mut self) -> Result<Event, RecvError> {
        self.rx.recv().await
    }
}

impl Drop for EventReceiver<'_> {
    fn drop(&mut self) {
        let listeners = self.distributor.listeners.clone();
        let stop_brodcasting = self.distributor.stop_broadcasting.clone();

        tokio::spawn(async move {
            let mut listeners = listeners.lock().await;
            *listeners -= 1;

            if *listeners == 0 {
                stop_brodcasting.notify_one();
            }
        });
    }
}
