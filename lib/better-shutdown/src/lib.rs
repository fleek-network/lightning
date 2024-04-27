#![feature(sync_unsafe_cell, thread_sleep_until)]

mod completion_fut;
mod controller;
mod rng;
mod shared;
mod signal_fut;
mod signal_own_fut;
mod wait_list;

use std::time::Duration;

pub use completion_fut::CompletionFuture;
pub use controller::{ShutdownController, ShutdownWaiter};
pub use signal_fut::ShutdownSignal;
pub use signal_own_fut::OwnedShutdownSignal;

#[test]
fn demo() {
    use std::cell::SyncUnsafeCell;

    static UNSAFE_CELL: SyncUnsafeCell<usize> = SyncUnsafeCell::new(0);

    let deadline = std::time::Instant::now() + Duration::from_secs(1);
    let result = (0..8)
        .map(|_| {
            std::thread::spawn(move || {
                std::thread::sleep_until(deadline);
                let mut collected = Vec::new();
                for _ in 0..100 {
                    let p = unsafe { &mut *UNSAFE_CELL.get() };
                    *p += 1;
                    collected.push(*p);
                }
                collected
            })
        })
        .map(|handle| handle.join().unwrap());

    for (i, collected) in result.enumerate() {
        println!("{i}: {collected:?}");
    }
}
