use futures::future::{BoxFuture, LocalBoxFuture};
use futures::stream::FuturesUnordered;
use futures::{Future, StreamExt};

type Cb = dyn Fn(BoxFuture<'static, ()>, Option<String>);

#[derive(Default)]
pub struct Executor {
    spawn_fn: Option<Box<Cb>>,
    futures: FuturesUnordered<LocalBoxFuture<'static, ()>>,
}

impl Executor {
    /// Set the spawn callback that the executor should proxy async tasks to.
    pub fn set_spawn_cb<F>(&mut self, spawn: F)
    where
        F: 'static + Fn(BoxFuture<'static, ()>, Option<String>),
    {
        if self.spawn_fn.is_some() {
            panic!("Executor spawn callback is already set.");
        }
        self.spawn_fn = Some(Box::new(spawn));
    }

    /// Insert the given future into this executor. Note that inserting the future does not drive it
    /// forward unless the executor is awaited on.
    pub fn spawn<F, O>(&self, fut: F, name: Option<String>)
    where
        F: Future<Output = O> + Send + 'static,
    {
        let f = self
            .spawn_fn
            .as_ref()
            .unwrap_or_else(|| panic!("Executor spawn callback is not set."));
        (f)(
            Box::pin(async move {
                fut.await;
            }),
            name,
        );
    }

    /// Run the executor until there is no more task to finish.
    pub async fn run_until_finish(&mut self) {
        while self.futures.next().await.is_some() {}
    }
}
