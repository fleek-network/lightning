use std::time::Duration;

pub struct Engine {
    /// The current time in this simulation.
    time: Duration,
    /// The next resource id.
    next_rid: u64,
}

pub struct Rid(u64);

struct RawConnection {}
