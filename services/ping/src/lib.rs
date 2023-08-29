//! This is meant to turn into a *very dumb* service to experiment with service loading.

#[no_mangle]
pub fn on_start() {}

#[no_mangle]
pub fn on_message() {}

#[no_mangle]
pub fn on_connected() {}

#[no_mangle]
pub fn on_disconnected() {}

#[no_mangle]
pub fn my_func() -> u32 {
    17
}
