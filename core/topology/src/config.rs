use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct Config {
    /// TESTING ONLY. Clustering target k value.
    pub testing_target_k: usize,
    /// TESTING ONLY. Minimum number of nodes to run the topology algorithm
    pub testing_min_nodes: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            testing_target_k: 8,
            testing_min_nodes: 9,
        }
    }
}
