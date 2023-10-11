use affair::AsyncWorker as WorkerTrait;
use async_trait::async_trait;
use lightning_interfaces::types::IndexRequest;

#[derive(Clone)]
pub struct IndexWorker {}

impl IndexWorker {
    pub fn new() -> Self {
        // todo
        Self {}
    }
}
#[async_trait]
impl WorkerTrait for IndexWorker {
    type Request = IndexRequest;
    type Response = ();

    async fn handle(&mut self, _req: Self::Request) -> Self::Response {
        // todo
    }
}
