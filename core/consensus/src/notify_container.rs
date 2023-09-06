//! A dumb way to get something from main to us.

use std::sync::Arc;

use tokio::sync::Notify;

static mut SENDER: Option<Arc<Notify>> = None;

pub fn set_value(value: Arc<Notify>) {
    // SAFETY: No other thread is calling this. It happens during init.
    unsafe {
        SENDER = Some(value);
    }
}

pub fn get_value() -> Arc<Notify> {
    unsafe { SENDER.as_ref().expect("value not set").clone() }
}
