use affair::Socket;
use fdi::BuildGraph;

use crate::collection::Collection;
use crate::types::DeliveryAcknowledgment;

/// The socket which upon receiving a delivery acknowledgment can add it to the aggregator
/// queue which will later roll up a batch of delivery acknowledgments to the consensus.
pub type DeliveryAcknowledgmentSocket = Socket<DeliveryAcknowledgment, ()>;

#[interfaces_proc::blank]
pub trait DeliveryAcknowledgmentAggregatorInterface<C: Collection>:
    BuildGraph + Sized + Send + Sync
{
    /// Returns the socket that can be used to submit delivery acknowledgments to be aggregated.
    #[socket]
    fn socket(&self) -> DeliveryAcknowledgmentSocket;
}

pub trait LaneManager {}
