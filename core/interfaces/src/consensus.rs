use fdi::BuildGraph;
use lightning_schema::LightningMessage;

use crate::components::NodeComponents;

#[interfaces_proc::blank]
pub trait ConsensusInterface<C: NodeComponents>: BuildGraph + Sized + Send + Sync {
    #[blank(())]
    type Certificate: LightningMessage + Clone;
}
