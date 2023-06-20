use std::{ops::Add, time::Duration};

use num::integer::Roots;

/// The statistic data for a ping measurement between two servers.
#[derive(Clone, Copy, Debug)]
pub struct PingStat {
    /// The minimum recorded ping.
    pub min: Duration,
    /// The average recorded ping.
    pub avg: Duration,
    /// The maximum recorded ping.
    pub max: Duration,
    /// The standard deviation of the recording.
    pub stddev: Duration,
    /// The number of runs.
    pub count: usize,
}

/// Merge two measurements into one measurement.
impl Add<PingStat> for PingStat {
    type Output = PingStat;

    fn add(self, rhs: PingStat) -> Self::Output {
        if self.count == 0 {
            return rhs;
        }

        if rhs.count == 0 {
            return self;
        }

        // The goal is to merge two standard deviations into one.
        //
        // Let's say data set one has standard deviation `s` and `N` data points.
        //
        // We observe:
        //
        // s        = √(∑(x_i - avg)^2 / N)
        // s^2      = ∑(x_i - avg)^2 / N
        // N * s^2  = ∑(x_i - avg)^2
        //          = ∑(x_i ^ 2 + avg^2 - 2 * x_i * avg)
        //          = N * avg^2 - 2avg∑x_i + ∑(x_i ^ 2)    since:
        //                                                   avg      = ∑x_i / N
        //                                                   ∑x_i     = avg * N
        //          = N * avg^2 - 2 * avg * avg * N + ∑(x_i ^ 2)
        //          = N * avg^2 - 2 * avg^2 * N + ∑(x_i ^ 2)
        //          = ∑(x_i ^ 2) - N * avg^2
        //          ^-- EQ-01
        //
        //  --> ∑(x_i ^ 2)  = N * s^2 + N * avg^2
        //                  = N * (s^2 + avg^2)
        //
        // The standard deviation of the merged data set one containing `N` element,
        // and the second one containing `M` elements can be computed as following.
        //
        // Notation:
        // `t`: the standard deviation of the second set.
        // `y_i`: the `i`-th member of the second set.
        // `o`: the stddev of the merged set.
        //
        // o^2              = (∑(x_i - merged_avg)^2 + ∑(y_i - merged_avg)^2) / (N + M)
        //
        // Via EQ-01 we can write:
        // (N + M) * o^2    = ∑(x_i ^ 2) +  ∑(y_i ^ 2) - (N + M) * merged_avg^2
        //      = N * (s^2 + avg_x^2) + M * (t^2 + avg_y^2) - (N + M) * merged_avg^2
        //      = N * (s^2 + avg_x^2) + M * (t^2 + avg_y^2) - (N + M) * merged_avg^2
        //
        // --> o
        //  = √( (N * (s^2 + avg_x^2) + M * (t^2 + avg_y^2)) / (N + M) -  merged_avg^2)
        //
        // and we know `merged_avg = (N * avg_x + M * avg_y) / (N + M)`.

        let n = self.count as u128;
        let avg_x = self.avg.as_micros();
        let s = self.stddev.as_micros();
        let m = rhs.count as u128;
        let avg_y = rhs.avg.as_micros();
        let t = rhs.stddev.as_micros();

        let merged_avg = (n * avg_x + m * avg_y) / (n + m);
        let o = n * (s * s + avg_x * avg_x) + m * (t * t + avg_y * avg_y);
        let o = o / (n + m);
        let o = o - merged_avg * merged_avg;
        let o = o.sqrt();

        let stddev = Duration::from_micros(o as u64);
        let avg = Duration::from_micros(merged_avg as u64);
        let min = self.min.min(rhs.min);
        let max = self.max.max(rhs.max);
        let count = self.count + rhs.count;

        Self {
            min,
            avg,
            max,
            stddev,
            count,
        }
    }
}
