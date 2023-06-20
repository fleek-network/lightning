use std::time::Duration;

use crate::ping::PingStat;

/// Set each frame to 250μs or forth of a millisecond.
pub const FRAME_DURATION: Duration = Duration::from_micros(250);

pub struct ServerId(usize);

/// The scene is where a simulation takes place.
pub struct Scene {}

/// The data provider returns the estimated statistics between different
/// servers and their peer to peer interaction with each other.
pub trait SceneDataProvider {
    /// Returns the ping data between the given source and destination.
    fn ping(&self, source: ServerId, destination: ServerId) -> PingStat;
}

impl Scene {
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
