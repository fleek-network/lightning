//! Simulon is a lite discrete event simulation engine used for simulating latency sensitive IO
//! bound applications. It provides a small set of API that can be used inside of a simulation
//! to interact with other nodes in the same simulation, and perform basic IO.

use std::time::Duration;

/// The api to use inside the simulation executor.
pub mod api;

/// The types and implementations around the latency data provider.
pub mod latency;

/// The types used for generating reports after a simulation.
pub mod report;

/// The simulator engine.
pub mod simulation;

mod future;
mod message;
mod state;
mod storage;

/// How many frames is one millisecond?
pub const FRAME_TO_MS: u64 = 4;

/// The duration of one frame.
pub const FRAME_DURATION: Duration = Duration::from_micros(1_000 / FRAME_TO_MS);
