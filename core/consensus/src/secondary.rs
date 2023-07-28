use freek_interfaces::PubSub;
use narwhal_config::{Committee, Parameters, WorkerCache};
use narwhal_executor::ExecutionState;
use narwhal_node::NodeStorage;

use self::types::PubSubMessage;

mod consensus;
mod pool;
mod types;

pub struct SecondaryService {}

impl SecondaryService {
    pub fn new(
        parameters: Parameters,
        store: NodeStorage,
        committee: Committee,
        worker_cache: WorkerCache,
    ) -> Self {
        todo!()
    }

    pub async fn start<State>(&self, state: State, pubsub: impl PubSub<PubSubMessage>)
    where
        State: ExecutionState + Send + Sync + 'static,
    {
        todo!()
    }

    pub async fn shutdown(&self) {}
}
