use std::fmt::Display;
use std::str::FromStr;

use anyhow::anyhow;
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
impl FromStr for TaskScope {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "local" => Ok(TaskScope::Local),
            "single" => Ok(TaskScope::Single),
            "cluster" => Ok(TaskScope::Cluster),
            _ => Err(anyhow!("unsupported scope")),
        }
    }
}
impl From<u8> for TaskScope {
    fn from(value: u8) -> Self {
        match value {
            0 => Self::Local,
            1 => Self::Single,
            2 => Self::Cluster,
            x => Self::Multicluster(x - 1),
        }
    }
}
impl From<TaskScope> for u8 {
    fn from(val: TaskScope) -> Self {
        match val {
            TaskScope::Local => 0,
            TaskScope::Single => 1,
            TaskScope::Cluster => 2,
            TaskScope::Multicluster(x) => x + 1,
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
impl ToDigest for TaskPayload {
    fn transcript(&self) -> TranscriptBuilder {
        match self {
            TaskPayload::Request(r) => r.transcript(),
            TaskPayload::Response(r) => r.transcript(),
        }
    }
}

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
