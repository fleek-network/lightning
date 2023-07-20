use futures::{future::LocalBoxFuture, Future};
use futures_task::{LocalFutureObj, LocalSpawn};

use crate::state::with_node;

/// Spawns the given future in its own task to be continued.
pub fn spawn<F>(future: F)
where
    F: Future<Output = ()> + 'static,
{
    let owned: LocalBoxFuture<()> = Box::pin(future);
    with_node(|n| {
        let future = LocalFutureObj::new(owned);
        n.spawn_pool
            .spawner()
            .spawn_local_obj(future)
            .expect("Failed to spawn future.");
    });
}
