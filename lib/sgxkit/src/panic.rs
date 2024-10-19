//! Internal panic handling utilities

use std::panic::PanicInfo;

use sgxkit_sys::fn0;

/// Utility to force clear and write a message to the output buffer
unsafe fn clear_and_write_output(msg: &str) {
    fn0::output_data_clear();
    fn0::output_data_append(msg.as_ptr() as usize, msg.len());
}

/// Clears output and writes "panicked at '$reason', src/main.rs:27:4"
pub fn default_panic_hook(info: &PanicInfo) {
    let string = info.to_string();

    // TODO: Should we have a special `trap` host function?
    //       Maybe to include as a bool in the signed output header
    unsafe { clear_and_write_output(&string) }
}

/// Set the default panic handler
pub fn set_default_hook() {
    std::panic::set_hook(Box::new(default_panic_hook));
}
