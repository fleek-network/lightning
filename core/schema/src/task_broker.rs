use std::fmt::Display;

use bytes::Bytes;
use fleek_crypto::NodeSignature;
use ink_quill::{ToDigest, TranscriptBuilder};
use lightning_types::{Digest, ServiceId};
use serde::{Deserialize, Serialize};

use crate::AutoImplSerde;

/// Scope to run a task under
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize, Serialize)]
pub enum TaskScope {
    /// Local scope explicitly ran on the current node.
    Local,
    /// Single node scope for a random node in the current cluster.
    Single,
    /// Cluster scope for duplicating a task with the current cluster
    Cluster,
    /// Multicluster scope for duplicating a task across multiple clusters
    Multicluster(u8),
}
impl Display for TaskScope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaskScope::Local => f.write_str("local"),
            TaskScope::Single => f.write_str("single"),
            TaskScope::Cluster => f.write_str("cluster"),
            TaskScope::Multicluster(n) => f.write_str(&format!("{n}-multicluster")),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub struct TaskMessage {
    pub payload: TaskPayload,
    pub signature: NodeSignature,
}

impl AutoImplSerde for TaskMessage {}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub enum TaskPayload {
    Request(TaskRequest),
    Response(TaskResponse),
}
impl AutoImplSerde for TaskPayload {}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub struct TaskRequest {
    pub service: ServiceId,
    pub timestamp: u64,
    pub payload: Bytes,
}
impl AutoImplSerde for TaskRequest {}
impl ToDigest for TaskRequest {
    fn transcript(&self) -> ink_quill::TranscriptBuilder {
        TranscriptBuilder::empty("FLEEK_TASK_REQUEST")
            .with("SERVICE_ID", &self.service)
            .with("TIMESTAMP", &self.timestamp)
            .with("PAYLOAD", &self.payload.as_ref())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub struct TaskResponse {
    pub request: Digest,
    pub timestamp: u64,
    pub payload: Bytes,
}
impl AutoImplSerde for TaskResponse {}
impl ToDigest for TaskResponse {
    fn transcript(&self) -> TranscriptBuilder {
        TranscriptBuilder::empty("FLEEK_TASK_RESPONSE")
            .with("REQUEST_DIGEST", &self.request)
            .with("TIMESTAMP", &self.timestamp)
            .with("payload", &self.payload.as_ref())
    }
}
