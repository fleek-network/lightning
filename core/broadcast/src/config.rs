use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    /// Time to live for the message cache, in minutes
    time_to_live_mins: usize,
    /// Time to idle for the message cache, in minutes
    time_to_idle_mins: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            time_to_live_mins: 60,
            time_to_idle_mins: 5,
        }
    }
}
