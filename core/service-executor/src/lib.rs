use std::marker::PhantomData;

use lightning_interfaces::infu_collection::Collection;

pub struct ServiceExecutor<C: Collection> {
    collection: PhantomData<C>,
}

#[test]
fn x() {
    use lightning_interfaces::Service;
    let _x = Service {
        start: fleek_service_ping_example::on_start,
        connected: fleek_service_ping_example::on_connected,
        disconnected: fleek_service_ping_example::on_disconnected,
        message: fleek_service_ping_example::on_message,
        respond: fleek_service_ping_example::on_event_response,
    };
}
