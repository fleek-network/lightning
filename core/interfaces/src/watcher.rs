use fdi::BuildGraph;

use crate::NodeComponents;

#[interfaces_proc::blank]
pub trait WatcherInterface<C: NodeComponents>: BuildGraph + Sized + Send + Sync {}
