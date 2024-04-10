use fdi::BuildGraph;

use crate::collection::Collection;

#[interfaces_proc::blank]
pub trait PingerInterface<C: Collection>: BuildGraph + Sized + Send + Sync {}
