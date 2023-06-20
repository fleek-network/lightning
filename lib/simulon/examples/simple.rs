#![allow(dead_code)]

use simulon::scene::ServerId;

fn node(_id: ServerId, ctx: &simulon::context::Context) {
    ctx.spawn(async {
        while let Some(conn) = ctx.accept(69).await {
            handle_connection(conn);
        }
    });

    todo!()
}

fn handle_connection(_conn: simulon::connection::Connection) {
    todo!()
}

fn main() {
    // init scene.
    // run node.
}
