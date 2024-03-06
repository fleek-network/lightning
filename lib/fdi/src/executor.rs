use futures::future::LocalBoxFuture;
use futures::stream::FuturesUnordered;
use futures::{Future, StreamExt};

#[derive(Default)]
pub struct Executor {
    futures: FuturesUnordered<LocalBoxFuture<'static, ()>>,
}

impl Executor {
    /// Insert the given future into this executor. Note that inserting the future does not drive it
    /// forward unless the executor is awaited on.
    pub fn spawn<F, O>(&mut self, fut: F)
    where
        F: Future<Output = O> + 'static,
    {
        self.futures.push(Box::pin(async move {
            fut.await;
        }));
    }

    /// Run the executor until there is no more task to finish.
    pub async fn run_until_finish(&mut self) {
        while self.futures.next().await.is_some() {}
    }
}
