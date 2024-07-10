use std::time::{SystemTime, UNIX_EPOCH};

use affair::{AsyncWorkerUnordered, Socket};
use anyhow::{Context, Result};
use bytes::{BufMut, BytesMut};
use fn_sdk::header::{write_header, ConnectionHeader};
use futures::StreamExt;
use lightning_interfaces::prelude::*;
use lightning_interfaces::schema::task_broker::{TaskRequest, TaskResponse};
use tokio_util::codec::{Framed, LengthDelimitedCodec};

pub type LocalTaskSocket = Socket<(u8, TaskRequest), anyhow::Result<TaskResponse>>;

/// Local task worker that acts as a simple client to services, executing a single request and
/// response under the task connection type. Output payloads are streamed back and returned as a
/// single block of data, and signed by the node public key.
pub struct LocalTaskWorker<P: ExecutorProviderInterface> {
    // For connecting to local services
    provider: P,
}

impl<P: ExecutorProviderInterface> LocalTaskWorker<P> {
    pub fn new(provider: P) -> Self {
        Self { provider }
    }
}

impl<P: ExecutorProviderInterface> AsyncWorkerUnordered for LocalTaskWorker<P> {
    type Request = (u8, TaskRequest);
    type Response = Result<TaskResponse>;
    async fn handle(&self, (depth, req): Self::Request) -> Self::Response {
        let digest = req.to_digest();

        // Connect to the service
        let mut stream = self
            .provider
            .connect(req.service)
            .await
            .context("failed to connect to service")?;

        // Write the header, containing the task payload inside the transport detail
        write_header(
            &ConnectionHeader {
                pk: None,
                transport_detail: fn_sdk::header::TransportDetail::Task {
                    depth,
                    payload: req.payload,
                },
            },
            &mut stream,
        )
        .await?;

        // Read and collect all payloads (similar behavior to http transport)
        let mut stream = Framed::new(stream, LengthDelimitedCodec::new());
        let mut payload = BytesMut::new();
        while let Some(Ok(chunk)) = stream.next().await {
            payload.put_slice(&chunk);
        }

        Ok(TaskResponse {
            request: digest,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            payload: payload.freeze(),
        })
    }
}
