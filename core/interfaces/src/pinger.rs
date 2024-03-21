use fdi::BuildGraph;

use crate::infu_collection::Collection;

#[infusion::service]
pub trait PingerInterface<C: Collection>: BuildGraph + Sized + Send + Sync {}
