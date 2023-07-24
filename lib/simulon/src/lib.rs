#![allow(dead_code)]

use std::time::Duration;

/// The api to use inside a node.
pub mod api;
pub mod latency;
pub mod report;
pub mod simulation;

mod future;
mod message;
mod state;
mod storage;

pub const FRAME_TO_MS: u64 = 4;
pub const FRAME_DURATION: Duration = Duration::from_micros(1_000 / FRAME_TO_MS);
