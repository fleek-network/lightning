use std::time::Duration;

use super::LatencyProvider;

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
