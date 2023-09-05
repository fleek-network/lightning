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
pub fn on_connected(args: OnConnectedArgs) {
    println!(
        "connection[{}]: connected '{}'",
        args.connection_id, args.client
    );
}

#[no_mangle]
pub fn on_message(args: OnMessageArgs) {
    println!(
        "connection[{}]: message '{:?}'",
        args.connection_id, args.payload
    );

    // send the hello world message.
    fn_sdk::api::connection_send(args.connection_id, b"Hello World".to_vec());
}

#[no_mangle]
pub fn on_disconnected(args: OnDisconnectedArgs) {
    println!("connection[{}]: disconnected", args.connection_id);
}
