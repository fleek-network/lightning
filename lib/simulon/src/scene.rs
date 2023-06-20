use std::time::Duration;

use crate::{bandwidth::Bandwidth, ping::PingStat};

/// How many frames equal 1ms?
pub const MS_IN_FRAMES: u64 = 4;

/// Set each frame to 250μs or forth of a millisecond.
pub const FRAME_DURATION: Duration = Duration::from_micros(250);

/// Default value used as the window size for TCP emulation.
pub const DEFAULT_TCP_WINDOW_SIZE: usize = 64 << 10;

/// Identifier for a single server.
#[derive(Hash, PartialEq, PartialOrd, Ord, Eq, Clone, Copy, Debug)]
pub struct ServerId(usize);

/// The scene is where a simulation takes place.
pub struct Scene {
    /// The current time.
    pub time: Duration,
}

/// An on-going transmission.
pub struct Transmission {
    /// The data being sent.
    pub data: Vec<u8>,
    /// The number of bits received so far.
    pub recivied: usize,
    /// The window size of this transmission.
    pub window_size: usize,
}

/// The data provider returns the estimated statistics between different
/// servers and their peer to peer interaction with each other.
pub trait SceneDataProvider {
    /// Returns the ping data between the given source and destination.
    fn ping(&self, source: ServerId, destination: ServerId) -> PingStat;

    /// Returns the bandwidth associated with the given server.
    fn bandwidth(&self, source: ServerId) -> Bandwidth;
}

impl Scene {
    /// Create and return a new scene.
    pub fn new() -> Self {
        Self {
            time: Duration::ZERO,
        }
    }

    /// Run the simulation for the given time. This is not the time it will take
    /// for this function to finish execution but it is rather the amount of time
    /// that will be simulated in this run. The simulation kicks of from where it
    /// was left off.
    pub fn run(&mut self, _duration: Duration) {
        todo!()
    }

    /// Render a single frame of the simulation.
    fn _render_frame(&mut self) {
        todo!()
    }
}

impl Default for Scene {
    fn default() -> Self {
        Self::new()
    }
}
