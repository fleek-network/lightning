#![allow(dead_code)]

use simulon::scene::ServerId;

fn node(_id: ServerId, ctx: &simulon::context::Context) {
    simulon::spawn::spawn(async {
        loop {
            let conn = ctx.accept(69).await;
            handle_connection(conn);
        }
    });

    todo!()
}

fn handle_connection(_conn: simulon::context::Connection) {
    todo!()
}

fn main() {
    // init scene.
    // run node.
}
