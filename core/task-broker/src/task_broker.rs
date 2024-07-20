use std::sync::{Arc, RwLock};

use affair::AsyncWorkerUnordered;
use anyhow::Result;
use fleek_crypto::{NodePublicKey, NodeSecretKey};
use futures::stream::FuturesUnordered;
use futures::{AsyncReadExt, StreamExt, TryStreamExt};
use lightning_interfaces::prelude::*;
use lightning_interfaces::{spawn_worker, RequestHeader, RequesterInterface, TaskError};
use lightning_metrics::increment_counter;
use rand::prelude::SliceRandom;
use rand::thread_rng;
use schema::task_broker::{TaskRequest, TaskResponse, TaskScope};
use serde::{Deserialize, Serialize};
use tokio::sync::watch;
use tokio::task::JoinSet;
use tracing::warn;
use types::NodeIndex;

use crate::local::{LocalTaskSocket, LocalTaskWorker};

pub struct TaskBroker<C: Collection> {
    socket: Arc<RwLock<Option<LocalTaskSocket>>>,
    requester: Arc<<C::PoolInterface as PoolInterface<C>>::Requester>,
    topology: tokio::sync::watch::Receiver<Arc<Vec<Vec<NodePublicKey>>>>,
    query_runner: c!(C::ApplicationInterface::SyncExecutor),
    max_depth: u8,
    // temp objects used for state loop
    temp: Option<(NodeSecretKey, c!(C::PoolInterface::Responder))>,
}

impl<C: Collection> Clone for TaskBroker<C> {
    fn clone(&self) -> Self {
        Self {
            max_depth: self.max_depth,
            socket: self.socket.clone(),
            requester: self.requester.clone(),
            topology: self.topology.clone(),
            query_runner: self.query_runner.clone(),
            temp: None,
        }
    }
}

impl<C: Collection> TaskBroker<C> {
    fn init(
        config: &C::ConfigProviderInterface,
        keystore: &C::KeystoreInterface,
        topology: &C::TopologyInterface,
        pool: &C::PoolInterface,
        fdi::Cloned(query_runner): fdi::Cloned<c!(C::ApplicationInterface::SyncExecutor)>,
    ) -> Result<Self> {
        let TaskBrokerConfig { max_depth } = config.get::<Self>();

        // Spawn state loop for handling pool and pubsub events
        let (req, responder) = pool.open_req_res(lightning_interfaces::ServiceScope::TaskBroker);

        Ok(Self {
            max_depth,
            socket: Arc::new(RwLock::new(None)),
            requester: req.into(),
            topology: topology.get_receiver(),
            query_runner,
            temp: Some((keystore.get_ed25519_sk(), responder)),
        })
    }

    fn post_init(
        &mut self,
        service_executor: &C::ServiceExecutorInterface,
        fdi::Cloned(shutdown): fdi::Cloned<ShutdownWaiter>,
    ) {
        let (key, responder) = self.temp.take().unwrap();

        // spawn worker to handle executing local tasks
        let provider = service_executor.get_provider();
        let waiter = shutdown.clone();
        let socket = spawn_worker!(
            LocalTaskWorker::new(provider),
            "TASK BROKER: Local task worker",
            waiter
        );

        *self.socket.write().unwrap() = Some(socket.clone());

        State::<C>::spawn(
            key,
            self.topology.clone(),
            responder,
            socket.clone(),
            shutdown,
        );
    }

    fn get_cluster(&self) -> Result<Vec<NodePublicKey>, TaskError> {
        // Get current topology cluster
        let topology = self.topology.borrow();
        let Some(nodes) = topology.first() else {
            return Err(TaskError::Internal("topology not initialized".to_string()));
        };
        if nodes.is_empty() {
            return Err(TaskError::Internal("no nodes in cluster".into()));
        }
        Ok(nodes.clone())
    }

    fn get_random_cluster_node(&self) -> Result<u32, TaskError> {
        // Select a random node within the local cluster
        let cluster = self.get_cluster()?;
        let pub_key = cluster.choose(&mut thread_rng()).unwrap();
        let idx = self
            .query_runner
            .pubkey_to_index(pub_key)
            .expect("topology should never ever give unknown node pubkeys/indecies");
        Ok(idx)
    }

    async fn run_task_on_peer(
        &self,
        task: TaskRequest,
        peer: NodeIndex,
    ) -> Result<TaskResponse, TaskError> {
        // Encode task and send the request
        let mut buf = Vec::new();
        task.encode(&mut buf)
            .map_err(|e| TaskError::Internal(e.to_string()))?;
        let res = self
            .requester
            .request(peer, buf.into())
            .await
            .map_err(|_| TaskError::Connect)?;

        // Stream and decode response
        let mut res = res.body().into_async_read();
        let mut buf = Vec::new();
        res.read_to_end(&mut buf)
            .await
            .map_err(|_| TaskError::InvalidResponse)?;

        TaskResponse::decode(&buf).map_err(|_| TaskError::InvalidResponse)
    }

    /// Get a random peer and run the task on it
    async fn run_single_task(&self, task: TaskRequest) -> Result<TaskResponse, TaskError> {
        let idx = self.get_random_cluster_node()?;
        self.run_task_on_peer(task, idx).await
    }

    /// Get peers in the cluster and run the task on them
    /// TODO: Collect 2/3 of the same responses and respond early, pruning invalid ones.
    async fn run_cluster_task(&self, task: TaskRequest) -> Vec<Result<TaskResponse, TaskError>> {
        match self.get_cluster() {
            Ok(c) => {
                c.iter()
                    .map(|v| {
                        self.query_runner
                            .pubkey_to_index(v)
                            .expect("topology should never give unknown node pubkeys/indecies")
                    })
                    .map(|idx| self.run_task_on_peer(task.clone(), idx))
                    .collect::<FuturesUnordered<_>>()
                    .collect::<Vec<_>>()
                    .await
            },
            Err(e) => vec![Err(e)],
        }
    }
}

impl<C: Collection> fdi::BuildGraph for TaskBroker<C> {
    fn build_graph() -> fdi::DependencyGraph {
        fdi::DependencyGraph::default()
            .with(Self::init.with_event_handler("_post", Self::post_init))
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct TaskBrokerConfig {
    max_depth: u8,
}
impl Default for TaskBrokerConfig {
    fn default() -> Self {
        Self { max_depth: 8 }
    }
}

impl<C: Collection> ConfigConsumer for TaskBroker<C> {
    const KEY: &'static str = "task_broker";
    type Config = TaskBrokerConfig;
}

impl<C: Collection> TaskBrokerInterface<C> for TaskBroker<C> {
    async fn run(
        &self,
        depth: u8,
        scope: TaskScope,
        task: TaskRequest,
    ) -> Vec<Result<TaskResponse, TaskError>> {
        if depth > self.max_depth {
            increment_counter!(
                "task_broker_task_blocked",
                Some("Number of task requests blocked due to reaching max depth")
            );
            let err = TaskError::MaxDepth(self.max_depth);
            warn!("{err}");
            return vec![Err(err)];
        }
        match scope {
            TaskScope::Local => {
                let socket = self
                    .socket
                    .read()
                    .expect("failed to access")
                    .as_ref()
                    .unwrap()
                    .clone();

                let res = socket
                    .run((depth, task))
                    .await
                    .expect("failed to run task")
                    .map_err(|e| {
                        task_failed_metric(scope);
                        TaskError::Internal(e.to_string())
                    });
                vec![res]
            },
            TaskScope::Single => {
                let res = self.run_single_task(task).await;
                if let Err(e) = &res {
                    warn!("Failed to run task on peer: {e}");
                    task_failed_metric(scope);
                }
                vec![res]
            },
            TaskScope::Cluster => self.run_cluster_task(task).await,
            TaskScope::Multicluster(_) => {
                unimplemented!(
                    "Multicluster task consensus not implemented (need multicluster aware broadcasts)"
                )
            },
        }
    }
}

fn task_failed_metric(scope: TaskScope) {
    let scope = scope.to_string();
    increment_counter!("task_broker_request_failed", Some("Task broker request failures per scope"), "scope" => scope.as_str())
}

/// State for handling incoming tasks
pub struct State<C: Collection> {
    // For finding single nodes within the cluster
    _topology: watch::Receiver<Arc<Vec<Vec<NodePublicKey>>>>,
    // Receive incoming tasks
    responder: c!(C::PoolInterface::Responder),
    // For executing tasks we receive locally
    socket: LocalTaskSocket,
    // Pending tasks we spawn
    pending_tasks: JoinSet<()>,
}

impl<C: Collection> State<C> {
    pub fn spawn(
        _sk: NodeSecretKey,
        _topology: watch::Receiver<Arc<Vec<Vec<NodePublicKey>>>>,
        responder: c!(C::PoolInterface::Responder),
        socket: LocalTaskSocket,
        shutdown: ShutdownWaiter,
    ) {
        // Hack to ensure the task set is never empty
        let mut set = JoinSet::new();
        set.spawn(futures::future::pending());

        let fut = Self {
            _topology,
            responder,
            socket,
            pending_tasks: set,
        }
        .run();

        spawn!(
            async move { shutdown.run_until_shutdown(fut).await },
            "TASK BROKER: Incoming task loop"
        );
    }

    async fn run(mut self) {
        loop {
            tokio::select! {
                // Drive pending tasks forward
                _ = self.pending_tasks.join_next() => {},
                // Incoming direct task requests
               Ok((header, responder)) = self.responder.get_next_request() => {
                    if let Err(e) = self.handle_incoming_pool_task(header, responder) {
                        warn!("Failed to handle incoming pool task: {e}");
                    }
                },
                else => {
                    break;
                }
            }
        }
    }

    fn handle_incoming_pool_task(
        &mut self,
        header: RequestHeader,
        mut handle: impl lightning_interfaces::RequestInterface,
    ) -> Result<()> {
        // Parse request payload
        let request = TaskRequest::decode(&header.bytes)?;

        let socket = self.socket.clone();
        self.pending_tasks.spawn(async move {
            match socket.run((0, request)).await {
                Ok(Ok(res)) => {
                    let mut bytes = Vec::new();
                    res.encode(&mut bytes).expect("failed to encode response");
                    if let Err(e) = handle.send(bytes.into()).await {
                        warn!("Failed to send task response: {e}");
                    }
                },
                Ok(Err(e)) => {
                    warn!("Failed to run task: {e}");
                    handle.reject(types::RejectReason::Other)
                },
                Err(e) => warn!("Failed to run task, socket failed: {e}"),
            };
        });

        Ok(())
    }
}
