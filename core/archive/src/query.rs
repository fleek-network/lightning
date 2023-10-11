use affair::AsyncWorker as WorkerTrait;
use async_trait::async_trait;
use lightning_interfaces::types::{ArchiveRequest, ArchiveResponse};

#[derive(Clone)]
pub struct QueryWorker {}

impl QueryWorker {
    pub fn new() -> Self {
        // todo
        Self {}
    }
}

#[async_trait]
impl WorkerTrait for QueryWorker {
    type Request = ArchiveRequest;
    type Response = ArchiveResponse;

    /// Implement your custom handler for this async worker and enjoy using `await`.
    async fn handle(&mut self, _req: Self::Request) -> Self::Response {
        todo!()
    }
}
