use std::marker::PhantomData;

use fn_sdk::internal::{
    OnConnectedArgs,
    OnDisconnectedArgs,
    OnEventResponseArgs,
    OnMessageArgs,
    OnStartArgs,
};
use lightning_interfaces::infu_collection::Collection;

pub struct Service {
    pub start: fn(OnStartArgs),
    pub connected: fn(OnConnectedArgs),
    pub disconnected: fn(OnDisconnectedArgs),
    pub message: fn(OnMessageArgs),
    pub respond: fn(OnEventResponseArgs),
}

pub struct ServiceExecutor<C: Collection> {
    collection: PhantomData<C>,
}

#[test]
fn x() {
    let _x = Service {
        start: fleek_service_ping_example::on_start,
        connected: fleek_service_ping_example::on_connected,
        disconnected: fleek_service_ping_example::on_disconnected,
        message: fleek_service_ping_example::on_message,
        respond: fleek_service_ping_example::on_event_response,
    };
}
