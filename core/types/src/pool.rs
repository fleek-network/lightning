use crate::NodeIndex;

#[derive(Clone, Copy, Debug)]
#[repr(u8)]
#[non_exhaustive]
pub enum RejectReason {
    TooManyRequests = 1,
    ContentNotFound = 2,
    Other = 3,
}

pub type BoxedFilterCallback = Box<dyn Fn(NodeIndex) -> bool + Send + Sync + 'static>;

pub enum Param<F = BoxedFilterCallback>
where
    F: Fn(NodeIndex) -> bool,
{
    Filter(F),
    Index(NodeIndex),
}

pub struct PeerFilter<F = BoxedFilterCallback>
where
    F: Fn(NodeIndex) -> bool,
{
    pub by_topology: bool,
    pub param: Param<F>,
}
