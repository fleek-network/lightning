use std::sync::Arc;

use lightning_interfaces::PubSub;
use mysten_metrics::RegistryService;
use narwhal_config::{Committee, Parameters, WorkerCache};
use narwhal_node::NodeStorage;

use self::consensus::EdgeConsensus;
use crate::{consensus::PubSubMsg, execution::Execution};

mod consensus;
mod pool;

pub struct EdgeService<P: PubSub<PubSubMsg> + 'static> {
    store: NodeStorage,
    committee: Option<Committee>,
    worker_cache: Option<WorkerCache>,
    consensus: Option<EdgeConsensus>,
    pub_sub: P,
    registry_service: Option<RegistryService>,
}

impl<P: PubSub<PubSubMsg> + 'static> EdgeService<P> {
    pub fn new(
        store: NodeStorage,
        committee: Committee,
        worker_cache: WorkerCache,
        pub_sub: P,
        registry_service: RegistryService,
    ) -> Self {
        Self {
            store,
            committee: Some(committee),
            worker_cache: Some(worker_cache),
            consensus: None,
            pub_sub,
            registry_service: Some(registry_service),
        }
    }

    pub async fn start(&mut self, state: Arc<Execution<P>>) {
        let consensus = EdgeConsensus::spawn(
            self.pub_sub.clone(),
            Parameters::default(),
            &self.store,
            self.committee
                .take()
                .expect("Tried starting edge service before calling new"),
            self.worker_cache
                .take()
                .expect("Tried starting edge service before calling new"),
            state,
            self.registry_service
                .take()
                .expect("Tried starting edge service before calling new"),
        );

        self.consensus = Some(consensus);
    }

    pub async fn shutdown(&mut self) {
        if let Some(consensus) = self.consensus.take() {
            consensus.shutdown().await;
        }
    }
}
