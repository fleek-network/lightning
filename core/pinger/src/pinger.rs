use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use lightning_interfaces::infu_collection::{c, Collection};
use lightning_interfaces::{
    ApplicationInterface,
    ConfigConsumer,
    PingerInterface,
    ReputationAggregatorInterface,
    WithStartAndShutdown,
};
use tokio::net::UdpSocket;
use tokio::sync::Notify;
use tracing::error;

use crate::config::Config;

pub struct Pinger<C: Collection> {
    config: Config,
    sender: Arc<PingSender<C>>,
    responder: Arc<PingResponder<C>>,
    is_running: Arc<AtomicBool>,
    shutdown_notify: Arc<Notify>,
}

impl<C: Collection> PingerInterface<C> for Pinger<C> {
    fn init(
        config: Self::Config,
        query_runner: c!(C::ApplicationInterface::SyncExecutor),
        rep_reporter: c!(C::ReputationAggregatorInterface::ReputationReporter),
    ) -> anyhow::Result<Self> {
        let shutdown_notify = Arc::new(Notify::new());
        let sender = PingSender::<C>::new(
            query_runner.clone(),
            rep_reporter.clone(),
            shutdown_notify.clone(),
        );
        let responder =
            PingResponder::<C>::new(query_runner, rep_reporter, shutdown_notify.clone());
        Ok(Self {
            config,
            sender: Arc::new(sender),
            responder: Arc::new(responder),
            is_running: Arc::new(AtomicBool::new(false)),
            shutdown_notify,
        })
    }
}

#[async_trait]
impl<C: Collection> WithStartAndShutdown for Pinger<C> {
    fn is_running(&self) -> bool {
        self.is_running.load(Ordering::Relaxed)
    }

    async fn start(&self) {
        // Note(matthias): providing the UPD socket to the sender and responder using interior
        // mutability is not elegant. It is necessary because the Pinger's init function is
        // not async, so we cannot create the socket in the init function and pass
        // it to the sender and responder there.
        // I didn't make the init function async because none of the other init functions are.
        if !self.is_running() {
            let sender = self.sender.clone();
            let responder = self.responder.clone();
            if sender.socket.lock().unwrap().is_none() {
                let socket = Arc::new(
                    UdpSocket::bind(self.config.address)
                        .await
                        .expect("Failed to bind to UDP socket"),
                );
                sender.provide_socket(socket.clone());
                responder.provide_socket(socket);
            }

            let is_running = self.is_running.clone();
            tokio::spawn(async move {
                sender.start().await;
                is_running.store(false, Ordering::Relaxed);
            });
            let is_running = self.is_running.clone();
            tokio::spawn(async move {
                responder.start().await;
                is_running.store(false, Ordering::Relaxed);
            });
            self.is_running.store(true, Ordering::Relaxed);
        } else {
            error!("Cannot start pinger because it is already running");
        }
    }

    async fn shutdown(&self) {
        self.shutdown_notify.notify_one();
    }
}

#[allow(unused)]
struct PingSender<C: Collection> {
    socket: Mutex<Option<Arc<UdpSocket>>>,
    query_runner: c!(C::ApplicationInterface::SyncExecutor),
    rep_reporter: c!(C::ReputationAggregatorInterface::ReputationReporter),
    shutdown_notify: Arc<Notify>,
}

impl<C: Collection> PingSender<C> {
    fn new(
        query_runner: c!(C::ApplicationInterface::SyncExecutor),
        rep_reporter: c!(C::ReputationAggregatorInterface::ReputationReporter),
        shutdown_notify: Arc<Notify>,
    ) -> Self {
        Self {
            socket: Mutex::new(None),
            query_runner,
            rep_reporter,
            shutdown_notify,
        }
    }

    async fn start(&self) {
        loop {
            tokio::select! {
                _ = self.shutdown_notify.notified() => {
                    break;
                }

            }
        }
    }

    fn provide_socket(&self, socket: Arc<UdpSocket>) {
        *self.socket.lock().unwrap() = Some(socket);
    }
}

#[allow(unused)]
struct PingResponder<C: Collection> {
    socket: Mutex<Option<Arc<UdpSocket>>>,
    query_runner: c!(C::ApplicationInterface::SyncExecutor),
    rep_reporter: c!(C::ReputationAggregatorInterface::ReputationReporter),
    shutdown_notify: Arc<Notify>,
}

impl<C: Collection> PingResponder<C> {
    fn new(
        query_runner: c!(C::ApplicationInterface::SyncExecutor),
        rep_reporter: c!(C::ReputationAggregatorInterface::ReputationReporter),
        shutdown_notify: Arc<Notify>,
    ) -> Self {
        Self {
            socket: Mutex::new(None),
            query_runner,
            rep_reporter,
            shutdown_notify,
        }
    }

    async fn start(&self) {
        loop {
            tokio::select! {
                _ = self.shutdown_notify.notified() => {
                    break;
                }

            }
        }
    }

    fn provide_socket(&self, socket: Arc<UdpSocket>) {
        *self.socket.lock().unwrap() = Some(socket);
    }
}

impl<C: Collection> ConfigConsumer for Pinger<C> {
    const KEY: &'static str = "pinger";

    type Config = Config;
}
