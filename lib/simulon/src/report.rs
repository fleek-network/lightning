use std::ops::{Add, Deref, DerefMut};

use derive_more::{Add, AddAssign};
use fxhash::FxHashMap;
use replace_with::replace_with_or_abort;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize, Add)]
pub struct Report {
    /// The number of simulated frames.
    pub frames: u64,
    /// The frame duration in nanoseconds.
    pub frame_duration: u128,
    /// The sum of metrics.
    pub total: Metrics,
    /// The log of different events.
    pub log: Log,
    /// The sum of the entire metrics per each 'n' frame.
    pub timeline: Timeline,
    /// Metrics for each node. During execution this must be empty.
    pub node: VecWithAdd<NodeMetrics>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Log {
    pub emitted: FxHashMap<String, FxHashMap<u128, u32>>,
}

impl Add for Log {
    type Output = Log;

    fn add(self, rhs: Self) -> Self::Output {
        assert!(rhs.emitted.is_empty());
        self
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, Add)]
pub struct NodeMetrics {
    /// The total metrics during the entire execution.
    pub total: Metrics,
    /// The metrics per each 'n' frame.
    pub timeline: Timeline,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, Add, AddAssign)]
pub struct Metrics {
    /// Amount of CPU processing time spent in nanoseconds.
    pub cpu_time: u128,
    /// Number of bytes sent out during this period.
    pub bytes_sent: u64,
    /// Number of messages sent out during this period.
    pub msg_sent: u32,
    /// Number of bytes received during this period. Bytes that are received are not
    /// always necessarily processed in the same period and might be queued for later.
    pub bytes_received: u64,
    /// Number of messages received during this period.
    pub msg_received: u32,
    /// Number of bytes processed during this period. This can be bytes that were queued or the
    /// ones that are received in the same period.
    pub bytes_processed: u64,
    /// Number of messages that have been processed and received by the executor.
    pub msg_processed: u32,
    /// Number of connections the node has accepted from other.
    pub connections_accepted: u16,
    /// Number of connections the node has requested to be established.
    pub connections_requested: u16,
    /// Number of connections that got closed during this frame.
    pub connections_closed: u16,
    /// Number of connections the node has refused from other.
    pub connections_refused: u16,
    /// Number of connections the node did not accept.
    pub connections_failed: u16,
}

impl NodeMetrics {
    #[inline(always)]
    pub fn insert(&mut self, key: Option<usize>, metric: Metrics) {
        replace_with_or_abort(&mut self.total, |m| m.add(metric));
        if let Some(key) = key {
            self.timeline.insert(key, metric);
        }
    }
}

impl Report {
    #[inline(always)]
    pub fn insert(&mut self, key: Option<usize>, metric: Metrics) {
        replace_with_or_abort(&mut self.total, |m| m.add(metric));
        if let Some(key) = key {
            self.timeline.insert(key, metric);
        }
    }
}

impl Metrics {
    pub fn is_empty(&self) -> bool {
        self.bytes_sent == 0
            && self.msg_sent == 0
            && self.bytes_received == 0
            && self.msg_received == 0
            && self.bytes_processed == 0
            && self.msg_processed == 0
            && self.connections_accepted == 0
            && self.connections_requested == 0
            && self.connections_closed == 0
            && self.connections_refused == 0
            && self.connections_failed == 0
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Timeline(pub FxHashMap<usize, Metrics>);

impl Timeline {
    pub fn insert(&mut self, key: usize, metric: Metrics) {
        if !metric.is_empty() {
            self.insert_inner(key, metric);
        }
    }

    #[inline(always)]
    fn insert_inner(&mut self, key: usize, metric: Metrics) {
        match self.entry(key) {
            std::collections::hash_map::Entry::Vacant(e) => {
                e.insert(metric);
            },
            std::collections::hash_map::Entry::Occupied(mut e) => {
                *e.get_mut() += metric;
            },
        }
    }
}

impl Add for Timeline {
    type Output = Self;

    fn add(mut self, rhs: Self) -> Self::Output {
        for (index, metric) in rhs.0 {
            self.insert_inner(index, metric);
        }

        self
    }
}

impl Deref for Timeline {
    type Target = FxHashMap<usize, Metrics>;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Timeline {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// A [`Vec`] wrapper that implements pairwise addition.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct VecWithAdd<T>(pub Vec<T>);

impl<T> Add for VecWithAdd<T>
where
    T: Add<Output = T>,
{
    type Output = VecWithAdd<T>;

    #[inline(always)]
    fn add(mut self, rhs: Self) -> Self::Output {
        if self.len() < rhs.len() {
            return rhs.add(self);
        }

        for (i, e) in rhs.0.into_iter().enumerate() {
            // you would really think an API this useful should be in the std, huh?
            replace_with_or_abort(&mut self.0[i], |v| v.add(e));
        }

        self
    }
}

impl<T> Deref for VecWithAdd<T> {
    type Target = Vec<T>;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> DerefMut for VecWithAdd<T> {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
