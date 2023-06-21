use std::time::Duration;

pub struct Engine {
    /// The current time in this simulation.
    time: Duration,
}

pub struct Rid(u64);

pub struct Message {
    buffer: Vec<u8>,
    transmitted: usize,
}

impl Engine {
    fn init() {}

    fn render_frame(&mut self) {}
}
