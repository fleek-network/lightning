use fdi::BuildGraph;

use crate::components::NodeComponents;

#[interfaces_proc::blank]
pub trait HandshakeInterface<C: NodeComponents>: BuildGraph + Sized + Send + Sync {}
