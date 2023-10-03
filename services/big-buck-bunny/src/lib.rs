use fn_sdk::internal::{
    OnConnectedArgs,
    OnDisconnectedArgs,
    OnEventResponseArgs,
    OnMessageArgs,
    OnStartArgs,
};
use hex_literal::hex;
use lazy_static::lazy_static;

// --- SDK Setup

pub fn on_start(args: OnStartArgs) {
    fn_sdk::api::setup(args);
}

pub fn on_event_response(args: OnEventResponseArgs) {
    fn_sdk::api::on_event_response(args);
}

// ---- END OF SDK SETUP --->

/// Blake3 hash of big buck bunny.
static HASH: [u8; 32] = hex!("1065b2fd8291ee2d37b490fa47791b1fc99043e0b3243456f221a4371b8c2bd1");
lazy_static! {
    static ref TREE: Option<fn_sdk::blockstore::ContentHandle> = {
        let Ok(tree) = fn_sdk::blockstore::ContentHandle::load_sync(&HASH) else {
            log::error!("How could we? To not have Big Buck Bunny is a crime.");
            return None;
        };

        Some(tree)
    };
}

pub fn on_connected(args: OnConnectedArgs) {
    if let Some(tree) = TREE.as_ref() {
        fn_sdk::api::connection_send(args.connection_id, (tree.len() as u32).to_be_bytes().into());
    } else {
        fn_sdk::api::connection_close(args.connection_id);
    }
}

pub fn on_message(args: OnMessageArgs) {
    if args.payload.len() != 4 {
        return;
    }

    let block = u32::from_be_bytes(*arrayref::array_ref![args.payload, 0, 4]) as usize;

    let Some(tree) = TREE.as_ref() else {
        return;
    };

    if block >= tree.len() {
        return;
    }

    log::info!("block {block}");

    fn_sdk::api::spawn(async move {
        let content = tree.get(block).await.unwrap();
        fn_sdk::api::connection_send(args.connection_id, content);
    });
}

pub fn on_disconnected(args: OnDisconnectedArgs) {
    println!("connection[{}]: disconnected", args.connection_id);
}
