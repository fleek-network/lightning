use fdi::BuildGraph;
use lightning_schema::LightningMessage;

use crate::collection::Collection;

#[interfaces_proc::blank]
pub trait ConsensusInterface<C: Collection>: BuildGraph + Sized + Send + Sync {
    #[blank(())]
    type Certificate: LightningMessage + Clone;
}
