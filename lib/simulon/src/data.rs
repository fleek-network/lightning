use crate::{bandwidth::Bandwidth, peer::PeerId, ping::PingStat};

/// The statistical data for peers and their interaction.
pub trait DataProvider {
    /// Returns the ping data between the given source and destination.
    fn ping(&self, source: PeerId, destination: PeerId) -> PingStat;

    /// Returns the bandwidth associated with the given peer for sending
    /// data.
    fn bandwidth_up(&self, source: PeerId) -> Bandwidth;

    /// Returns the bandwidth associated with the given peer for downloading
    /// data.
    fn bandwidth_down(&self, source: PeerId) -> Bandwidth;
}
