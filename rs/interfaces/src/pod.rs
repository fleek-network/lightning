use affair::Socket;
use async_trait::async_trait;

use crate::{common::WithStartAndShutdown, config::ConfigConsumer, consensus::MempoolPort};

/// The port which upon receiving a delivery acknowledgment can add it to the aggregator
/// queue which will later roll up a batch of delivery acknowledgments to the consensus.
pub type DeliveryAcknowledgmentPort = Socket<DeliveryAcknowledgment, ()>;

#[async_trait]
pub trait DeliveryAcknowledgmentAggregatorInterface:
    WithStartAndShutdown + ConfigConsumer + Sized + Send + Sync
{
    ///
    async fn init(config: Self::Config, consensus: MempoolPort) -> anyhow::Result<Self>;

    ///
    fn port(&self) -> DeliveryAcknowledgmentPort;
}

pub struct DeliveryAcknowledgment;
