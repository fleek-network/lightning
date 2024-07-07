use std::marker::PhantomData;
use std::sync::Arc;

use affair::AsyncWorkerUnordered;
use anyhow::Result;
use fleek_crypto::{NodePublicKey, NodeSecretKey};
use futures::{AsyncReadExt, TryStreamExt};
use lightning_interfaces::prelude::*;
use lightning_interfaces::{spawn_worker, RequestHeader, RequesterInterface, TaskError};
use lightning_metrics::increment_counter;
use rand::prelude::SliceRandom;
use rand::thread_rng;
use schema::task_broker::{TaskPayload, TaskRequest, TaskResponse, TaskScope};
use tokio::sync::watch;
use tokio::task::JoinSet;
use tracing::warn;

use crate::local::{LocalTaskSocket, LocalTaskWorker};

pub struct TaskBroker<C: Collection> {
    socket: LocalTaskSocket,
    requester: Arc<<C::PoolInterface as PoolInterface<C>>::Requester>,
    topology: tokio::sync::watch::Receiver<Arc<Vec<Vec<NodePublicKey>>>>,
    query_runner: c!(C::ApplicationInterface::SyncExecutor),
    _p: PhantomData<C>,
}

impl<C: Collection> Clone for TaskBroker<C> {
    fn clone(&self) -> Self {
        Self {
            socket: self.socket.clone(),
            requester: self.requester.clone(),
            topology: self.topology.clone(),
            query_runner: self.query_runner.clone(),
            _p: self._p,
        }
    }
}

impl<C: Collection> TaskBroker<C> {
    fn init(
        keystore: &C::KeystoreInterface,
        service_executor: &C::ServiceExecutorInterface,
        topology: &C::TopologyInterface,
        pool: &C::PoolInterface,
        broadcast: &C::BroadcastInterface,
        fdi::Cloned(query_runner): fdi::Cloned<c!(C::ApplicationInterface::SyncExecutor)>,
        fdi::Cloned(shutdown): fdi::Cloned<ShutdownWaiter>,
    ) -> Result<Self> {
        // spawn worker to handle executing local tasks
        let provider = service_executor.get_provider();
        let waiter = shutdown.clone();
        let socket = spawn_worker!(
            LocalTaskWorker::new(provider),
            "TASK BROKER: Local task worker",
            waiter
        );

        // Spawn state loop for handling pool and pubsub events
        let (req, res) = pool.open_req_res(lightning_interfaces::ServiceScope::TaskBroker);
        let pubsub = broadcast.get_pubsub::<TaskPayload>(types::Topic::TaskBroker);
        State::<C>::new(
            keystore.get_ed25519_sk(),
            pubsub,
            topology.get_receiver(),
            res,
            socket.clone(),
            shutdown,
        )
        .spawn();

        Ok(Self {
            socket,
            requester: req.into(),
            topology: topology.get_receiver(),
            query_runner,
            _p: PhantomData,
        })
    }

    async fn run_single_node_task(&self, task: TaskRequest) -> Result<TaskResponse, TaskError> {
        let id = {
            // Get current topology
            let topology = self.topology.borrow();
            let Some(nodes) = topology.get(0) else {
                return Err(TaskError::Internal("topology not initialized".to_string()));
            };
            if nodes.is_empty() {
                return Err(TaskError::Internal("no nodes in cluster".into()));
            }

            // Select a random node within the local cluster
            let pub_key = { nodes.choose(&mut thread_rng()).unwrap() };
            self.query_runner
                .pubkey_to_index(&pub_key)
                .expect("topology should never ever give unknown node pubkeys/indecies")
        };

        // Encode task and send the request
        let mut buf = Vec::new();
        task.encode(&mut buf)
            .map_err(|e| TaskError::Internal(e.to_string()))?;
        let res = self
            .requester
            .request(id, buf.into())
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
}

impl<C: Collection> fdi::BuildGraph for TaskBroker<C> {
    fn build_graph() -> fdi::DependencyGraph {
        fdi::DependencyGraph::default().with(Self::init)
    }
}

impl<C: Collection> TaskBrokerInterface<C> for TaskBroker<C> {
    async fn run(
        &self,
        scope: TaskScope,
        task: TaskRequest,
    ) -> Vec<Result<TaskResponse, TaskError>> {
        match scope {
            TaskScope::Local => vec![
                self.socket
                    .run(task)
                    .await
                    .expect("failed to run task")
                    .map_err(|e| {
                        task_failed_metric(scope);
                        TaskError::Internal(e.to_string())
                    }),
            ],
            TaskScope::Single => {
                let res = self.run_single_node_task(task).await;
                if let Err(e) = &res {
                    warn!("Failed to run task on peer: {e}");
                    task_failed_metric(scope);
                }
                vec![res]
            },
            TaskScope::Cluster => unimplemented!(
                "Cluster task consensus not implemented (need cluster aware broadcasts)"
            ),
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
    // For sending and receiving task requests to the network
    pubsub: c!(C::BroadcastInterface::PubSub<TaskPayload>),
    // For finding single nodes within the cluster
    _topology: watch::Receiver<Arc<Vec<Vec<NodePublicKey>>>>,
    // Receive incoming tasks
    responder: c!(C::PoolInterface::Responder),
    // For executing tasks we receive locally
    socket: LocalTaskSocket,
    // Pending tasks we spawn
    pending_tasks: JoinSet<()>,
    shutdown: ShutdownWaiter,
}

impl<C: Collection> State<C> {
    pub fn new(
        _sk: NodeSecretKey,
        pubsub: c!(C::BroadcastInterface::PubSub<TaskPayload>),
        _topology: watch::Receiver<Arc<Vec<Vec<NodePublicKey>>>>,
        responder: c!(C::PoolInterface::Responder),
        socket: LocalTaskSocket,
        shutdown: ShutdownWaiter,
    ) -> Self {
        let mut set = JoinSet::new();
        // Hack to ensure the set is never empty
        set.spawn(futures::future::pending());

        Self {
            pubsub,
            _topology,
            responder,
            socket,
            pending_tasks: set,
            shutdown,
        }
    }

    pub fn spawn(self) {
        spawn!(
            async move { self.shutdown.clone().run_until_shutdown(self.run()).await },
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
                // Incoming pubsub messages
                Some(payload) = self.pubsub.recv() => {
                    if let Err(e) = self.handle_incoming_pubsub_task(payload) {
                        warn!("Failed to handle incoming pubsub payload: {e}");
                    }
                }
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
            match socket.run(request).await {
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

    fn handle_incoming_pubsub_task(&mut self, _payload: TaskPayload) -> Result<()> {
        unimplemented!("todo: handle incoming reqs for cluster consensus");
    }
}
