use fdi::BuildGraph;
use futures::Future;

use crate::schema::task_broker::{TaskRequest, TaskResponse, TaskScope};
use crate::Collection;

#[derive(Debug, Clone, thiserror::Error)]
pub enum TaskError {
    #[error("Internal error running task: {_0:?}")]
    Internal(String),
    #[error("Failed to connect to peer")]
    Connect,
    #[error("Peer timeout running task")]
    Timeout,
    #[error("Peer disconnected running task")]
    PeerDisconnect,
    #[error("Peer sent invalid response")]
    InvalidResponse,
    #[error("Maximum depth reached: {_0:?}")]
    MaxDepth(u8),
}

/// Task broker for services on the node.
///
/// Includes an internal service client allowing dispatching requests to
/// various scopes of tasks.
#[interfaces_proc::blank]
pub trait TaskBrokerInterface<C: Collection>: BuildGraph + Clone + Send + Sync + 'static {
    /// Run a task in a given scope, returning the (collection of) responses for the task.
    /// For services and use cases that allow recursion (ie, runtimes that expose running
    /// subtasks), depth should be used to stop requests
    #[blank = async { vec![] }]
    fn run(
        &self,

        depth: u8,
        scope: TaskScope,
        task: TaskRequest,
    ) -> impl Future<Output = Vec<Result<TaskResponse, TaskError>>> + Send;
}
