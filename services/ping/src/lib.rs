use arrayref::array_ref;
use dashmap::DashMap;
use fn_sdk::api::{connection_close, connection_send};
use fn_sdk::blockstore::ContentHandle;
use fn_sdk::internal::{
    OnConnectedArgs,
    OnDisconnectedArgs,
    OnEventResponseArgs,
    OnMessageArgs,
    OnStartArgs,
};
use fxhash::FxBuildHasher;
use lazy_static::lazy_static;

// --- SDK Setup

pub fn on_start(args: OnStartArgs) {
    fn_sdk::api::setup(args);
}

pub fn on_event_response(args: OnEventResponseArgs) {
    fn_sdk::api::on_event_response(args);
}

// ---- END OF SDK SETUP --->

lazy_static! {
    static ref CONNECTIONS: DashMap<u64, Connection, FxBuildHasher> =
        DashMap::with_capacity_and_hasher(1024, FxBuildHasher::default());
}

// our webRTC transport does not handle >64KB messages.
const CHUNK_SIZE: usize = 32 * 1024;

#[derive(Default)]
struct Connection {
    tree: Option<ContentHandle>,
    cursor: usize,
}

pub fn on_connected(args: OnConnectedArgs) {
    CONNECTIONS.insert(args.connection_id, Connection::default());
}

pub fn on_message(args: OnMessageArgs) {
    fn_sdk::api::spawn(async move {
        let Some(mut conn) = CONNECTIONS.get_mut(&args.connection_id) else {
            log::error!("connection not found.");
            return;
        };

        let connection: &mut Connection = &mut conn;

        let tree = connection.tree.as_ref();
        let cursor = &mut connection.cursor;

        let Some(tree) = tree else {
            if args.payload.len() != 32 {
                connection_close(args.connection_id);
                return;
            }

            let Ok(tree) = ContentHandle::load(array_ref![args.payload, 0, 32]).await else {
                connection_send(args.connection_id, "NOTFOUND".into());
                connection_close(args.connection_id);
                return;
            };

            conn.tree = Some(tree);
            return;
        };

        let block_counter = *cursor;
        if block_counter >= tree.len() {
            connection_close(args.connection_id);
            return;
        }
        *cursor += 1;

        let Ok(content) = tree.get(block_counter).await else {
            connection_close(args.connection_id);
            return;
        };

        for (i, chunk) in content.chunks(CHUNK_SIZE).enumerate() {
            let mut vec = chunk.to_vec();
            vec.push(i as u8);
            fn_sdk::api::connection_send(args.connection_id, vec);
        }
    });
}

pub fn on_disconnected(args: OnDisconnectedArgs) {
    CONNECTIONS.remove(&args.connection_id);
}
