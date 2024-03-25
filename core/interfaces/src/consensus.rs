use fdi::BuildGraph;
use lightning_schema::LightningMessage;

use crate::infu_collection::Collection;

#[infusion::service]
pub trait ConsensusInterface<C: Collection>: BuildGraph + Sized + Send + Sync {
    type Certificate: LightningMessage + Clone;
}
