use std::collections::{HashMap, VecDeque};

use lightning_interfaces::infu_collection::Collection;
use lightning_interfaces::types::{Blake3Hash, ImmutablePointer, OriginProvider};
use lightning_interfaces::{OriginProviderSocket, ResolverInterface};
use tokio::sync::{broadcast, mpsc, oneshot};
use tokio::task::JoinSet;
use tracing::error;

use crate::fetcher::Uri;

pub struct OriginFetcher<C: Collection> {
    tasks: JoinSet<Result<SuccessResponse, ErrorResponse>>,
    queue: VecDeque<ImmutablePointer>,
    rx: mpsc::Receiver<OriginRequest>,
    origin_socket: OriginProviderSocket,
    resolver: C::ResolverInterface,
    capacity: usize,
    shutdown_rx: oneshot::Receiver<()>,
}

impl<C: Collection> OriginFetcher<C> {
    pub fn new(
        capacity: usize,
        origin_socket: OriginProviderSocket,
        rx: mpsc::Receiver<OriginRequest>,
        resolver: C::ResolverInterface,
        shutdown_rx: oneshot::Receiver<()>,
    ) -> Self {
        Self {
            tasks: JoinSet::new(),
            queue: VecDeque::new(),
            rx,
            origin_socket,
            resolver,
            capacity,
            shutdown_rx,
        }
    }

    pub async fn start(mut self) {
        let mut pending_requests: HashMap<Uri, broadcast::Sender<Result<Blake3Hash, OriginError>>> =
            HashMap::new();
        loop {
            tokio::select! {
                _ = &mut self.shutdown_rx => {
                    break;
                }
                request = self.rx.recv() => {
                    if let Some(request) = request {
                        let uri = request.pointer.uri.clone();
                        let rx = if let Some(tx) = pending_requests.get(&uri) {
                            // If a request for this uri is currently pending, subscribe to get
                            // notified about the result.
                            tx.subscribe()
                        } else {
                            // If no request for this uri currently exists, create new request.
                            if self.tasks.len() < self.capacity {
                                self.spawn(request.pointer).await;
                            } else {
                                self.queue.push_back(request.pointer);
                            }
                            let (tx, rx) = broadcast::channel(1);
                            pending_requests.insert(uri, tx);
                            rx
                        };
                        request.response.send(rx).expect("Failed to send response");
                    }
                }
                Some(res) = self.tasks.join_next() => {
                    match res {
                        Ok(Ok(SuccessResponse { pointer, hash })) => {
                            let uri = pointer.uri.clone();
                            self.resolver.publish(hash, &[pointer]).await;
                            if let Some(tx) = pending_requests.remove(&uri) {
                                tx.send(Ok(hash)).expect("Failed to send hash");
                            }
                        }
                        Ok(Err(e)) => {
                            match e {
                                ErrorResponse::OriginSocketError => error!("Failed to get response from socket"),
                                ErrorResponse::OriginFetchError(uri) => {
                                    if let Some(tx) = pending_requests.remove(&uri) {
                                        tx.send(Err(OriginError)).expect("Failed to send response");
                                    }
                                    error!("Failed to fetch data from origin");
                                },
                            }
                        },
                        Err(e) => error!("Failed to join task: {e:?}"),
                    }
                    if self.tasks.len() < self.capacity {
                        if let Some(pointer) = self.queue.pop_front() {
                            self.spawn(pointer).await;
                        }
                    }
                }
            }
        }
    }

    async fn spawn(&mut self, pointer: ImmutablePointer) {
        let origin_socket = self.origin_socket.clone();
        self.tasks.spawn(async move {
            match &pointer.origin {
                OriginProvider::IPFS => match origin_socket.run(pointer.uri.clone()).await {
                    Ok(Ok(hash)) => Ok(SuccessResponse { pointer, hash }),
                    Ok(Err(e)) => {
                        error!("Failed to fetch data: {e:?}");
                        Err(ErrorResponse::OriginFetchError(pointer.uri))
                    },
                    Err(_) => Err(ErrorResponse::OriginSocketError),
                },
                _ => unreachable!(),
            }
        });
    }
}

pub struct OriginRequest {
    pub pointer: ImmutablePointer,
    pub response: oneshot::Sender<broadcast::Receiver<Result<Blake3Hash, OriginError>>>,
}

struct SuccessResponse {
    pointer: ImmutablePointer,
    hash: Blake3Hash,
}

#[derive(Debug, thiserror::Error)]
enum ErrorResponse {
    #[error("Failed to get message from origin socket")]
    OriginSocketError,
    #[error("Failed to fetch data from origin: {0:?}")]
    OriginFetchError(Uri),
}

#[derive(Debug, Clone, thiserror::Error)]
#[error("Failed to fetch data from origin")]
pub struct OriginError;
