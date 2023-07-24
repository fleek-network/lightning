use std::time::Duration;

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

/// A latency provider that always returns the same value.
#[derive(Clone, Copy)]
pub struct ConstLatencyProvider(Duration);

impl From<Duration> for ConstLatencyProvider {
    #[inline]
    fn from(value: Duration) -> Self {
        Self(value)
    }
}

impl ConstLatencyProvider {
    #[inline]
    fn new(duration: Duration) -> Self {
        Self(duration)
    }
}

impl Default for ConstLatencyProvider {
    #[inline]
    fn default() -> Self {
        Self::new(Duration::from_millis(1))
    }
}

impl LatencyProvider for ConstLatencyProvider {
    fn get(&mut self, _a: usize, _b: usize) -> Duration {
        self.0
    }
}

/// A latency provider with pre-filled real world ping data.
#[derive(Default)]
pub struct DefaultLatencyProvider {}

impl LatencyProvider for DefaultLatencyProvider {
    fn get(&mut self, _a: usize, _b: usize) -> Duration {
        Duration::from_millis(1)
    }
}
