use std::sync::Arc;

use lightning_interfaces::PubSub;
use narwhal_config::Committee;

use self::consensus::EdgeConsensus;
use crate::consensus::PubSubMsg;
use crate::execution::Execution;

mod consensus;
mod transaction_store;

pub struct EdgeService<P: PubSub<PubSubMsg> + 'static> {
    committee: Option<Committee>,
    consensus: Option<EdgeConsensus>,
    pub_sub: P,
}

impl<P: PubSub<PubSubMsg> + 'static> EdgeService<P> {
    pub fn new(committee: Committee, pub_sub: P) -> Self {
        Self {
            committee: Some(committee),
            consensus: None,
            pub_sub,
        }
    }

    pub async fn start(&mut self, state: Arc<Execution<P>>) {
        let consensus = EdgeConsensus::spawn(
            self.pub_sub.clone(),
            self.committee
                .take()
                .expect("Tried starting edge service before calling new"),
            state,
        );

        self.consensus = Some(consensus);
    }

    pub async fn shutdown(&mut self) {
        if let Some(consensus) = self.consensus.take() {
            consensus.shutdown().await;
        }
    }
}
