use crate::identity::PeerId;

#[derive(Clone, Copy, Eq, PartialEq, PartialOrd, Ord)]
pub enum Weight {
    /// It's good to know, but not crucial information.
    Relaxed,
    /// A strong behavior.
    Strong,
    /// Has the strongest weight, usually used for provably wrong behavior.
    Provable,
}

/// A singleton object that can be called from anywhere during the execution of the program,
/// by any part of the codebase, and is responsible for collecting information about a certain
/// peer and record our interactions with it.
///
/// # Implementation Notes
///
/// These functions should be callable from any thread, and it is expected to support many
/// non-blocking calls. Synchronization between threads should be minimal and the use of
/// locks is not recommended.
pub trait LocalReputationCollector {
    /// Performs any initialization steps required. Calling this function multiple times
    /// should be the same as calling it once. It is expected to be called from our main
    /// during the program's global initialization.
    fn setup();

    ///
    fn report_sat(peer_id: &PeerId, weight: Weight);

    ///
    fn report_unsat(peer_id: &PeerId, weight: Weight);

    /// Return a signed number indicating our trust level towards a peer.
    fn get(peer_id: &PeerId) -> i64;
}
