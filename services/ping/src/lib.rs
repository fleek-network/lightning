//! This is meant to turn into a *very dumb* service to experiment with service loading.
use fn_sdk::internal::{
    OnConnectedArgs,
    OnDisconnectedArgs,
    OnEventResponseArgs,
    OnMessageArgs,
    OnStartArgs,
};

// --- SDK Setup

#[no_mangle]
pub fn on_start(args: OnStartArgs) {
    fn_sdk::api::setup(args);
}

#[no_mangle]
pub fn on_event_response(args: OnEventResponseArgs) {
    fn_sdk::api::on_event_response(args);
}

// ---- END OF SDK SETUP --->

#[no_mangle]
pub fn on_message(_args: OnMessageArgs) {}

#[no_mangle]
pub fn on_connected(_args: OnConnectedArgs) {}

#[no_mangle]
pub fn on_disconnected(_args: OnDisconnectedArgs) {}
