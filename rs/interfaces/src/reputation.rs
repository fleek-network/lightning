#[derive(Clone, Copy, Eq, PartialEq, PartialOrd, Ord)]
pub enum Weight {
    /// It's good to know, but not crucial information.
    Relaxed,
    /// A strong behavior.
    Strong,
    /// Has the strongest weight, usually used for provably wrong behavior.
    Provable,
}

pub trait ReputationAggregatorInterface: Clone {}
