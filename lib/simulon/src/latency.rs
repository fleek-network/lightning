use std::time::Duration;

mod constant;
pub mod ping;

/// The latency provider is instantiated per simulation and is responsible to provide the
/// latency/distance value for two nodes.
pub trait LatencyProvider: Default {
    /// Called once right before the simulation is started with the number of nodes that are being
    /// simulated.
    ///
    /// This should perform any initialization necessary.
    fn init(&mut self, _number_of_nodes: usize) {}

    /// Return a latency between two nodes from the provided global indices.
    fn get(&mut self, a: usize, b: usize) -> Duration;
}

pub use constant::ConstLatencyProvider;
pub use ping::PingDataLatencyProvider;

pub type DefaultLatencyProvider = PingDataLatencyProvider;
