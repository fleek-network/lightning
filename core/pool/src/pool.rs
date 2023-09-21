use std::marker::PhantomData;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use lightning_interfaces::infu_collection::{c, Collection};
use lightning_interfaces::{ConfigConsumer, PoolInterface, SignerInterface, WithStartAndShutdown};
use log::error;
use netkit::builder::Builder;
use netkit::endpoint::{Endpoint, Event, Request, ServiceScope};
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::Notify;
use tokio::task::JoinHandle;

use crate::config::Config;

pub struct Pool<C: Collection> {
    endpoint: Mutex<Option<Endpoint>>,
    handle: Mutex<Option<JoinHandle<Endpoint>>>,
    shutdown_notify: Arc<Notify>,
    _marker: PhantomData<C>,
}

impl<C: Collection> PoolInterface<C> for Pool<C> {
    fn init(config: Self::Config, signer: &c!(C::SignerInterface)) -> anyhow::Result<Self> {
        let (_, sk) = signer.get_sk();
        let mut builder = Builder::new(sk);
        builder.socket_address(config.address);
        builder.keep_alive_interval(config.keep_alive_interval);
        let endpoint = builder.build()?;
        let shutdown_notify = Arc::new(Notify::new());

        Ok(Self {
            endpoint: Mutex::new(Some(endpoint)),
            handle: Mutex::new(None),
            shutdown_notify,
            _marker: PhantomData,
        })
    }

    fn network_event_receiver(&self) -> (ServiceScope, Receiver<Event>) {
        if let Some(endpoint) = self.endpoint.lock().unwrap().as_mut() {
            endpoint.network_event_receiver()
        } else {
            panic!("`network_event_receiver` was called after starting the pool");
        }
    }

    fn request_sender(&self) -> Sender<Request> {
        if let Some(endpoint) = self.endpoint.lock().unwrap().as_ref() {
            endpoint.request_sender()
        } else {
            panic!("`request_sender` was called after starting the pool");
        }
    }
}

#[async_trait]
impl<C: Collection> WithStartAndShutdown for Pool<C> {
    fn is_running(&self) -> bool {
        self.endpoint.lock().unwrap().is_none()
    }

    async fn start(&self) {
        if let Some(mut endpoint) = self.endpoint.lock().unwrap().take() {
            let shutdown_notify = self.shutdown_notify.clone();
            let handle = tokio::spawn(async move {
                loop {
                    tokio::select! {
                        _ = shutdown_notify.notified() => {
                            break;
                        }
                        _ = endpoint.start() => {
                            break
                        }
                    }
                }
                endpoint
            });
            *self.handle.lock().unwrap() = Some(handle);
        } else {
            error!("Cannot start pool because it is already running");
        }
    }

    async fn shutdown(&self) {
        self.shutdown_notify.notify_one();
        let endpoint_guard = self.handle.lock().unwrap().take().unwrap();
        let endpoint = endpoint_guard.await.unwrap();
        *self.endpoint.lock().unwrap() = Some(endpoint);
    }
}

impl<C: Collection> ConfigConsumer for Pool<C> {
    const KEY: &'static str = "pool";

    type Config = Config;
}
