//! This is meant to turn into a *very dumb* service to experiment with service loading.
use fn_sdk::internal::{
    OnConnectedArgs,
    OnDisconnectedArgs,
    OnEventResponseArgs,
    OnMessageArgs,
    OnStartArgs,
};

#[no_mangle]
pub fn on_start(_args: OnStartArgs) {}

#[no_mangle]
pub fn on_message(_args: OnMessageArgs) {}

#[no_mangle]
pub fn on_connected(_args: OnConnectedArgs) {}

#[no_mangle]
pub fn on_disconnected(_args: OnDisconnectedArgs) {}

#[no_mangle]
pub fn on_event_response(_args: OnEventResponseArgs) {}
