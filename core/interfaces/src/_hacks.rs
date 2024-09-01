use std::any::type_name;
use std::marker::PhantomData;

use affair::{AsyncWorkerUnordered, Socket};
use fdi::BuildGraph;

use crate::{ConfigConsumer, ConfigProviderInterface, NodeComponents};

/// The magical type that implements all of the traits.
#[derive(Clone, Default, Copy)]
pub struct Blanket;

impl BuildGraph for Blanket {
    fn build_graph() -> fdi::DependencyGraph {
        fdi::DependencyGraph::new()
    }
}

impl tokio_stream::Stream for Blanket {
    type Item = Result<bytes::Bytes, std::io::Error>;

    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        todo!()
    }
}

pub struct AsValue<T>(PhantomData<T>);

impl<T> Default for AsValue<T> {
    fn default() -> Self {
        Self(PhantomData)
    }
}

pub trait ConfigConsumerProxy {
    /// Request the config of Self if Self: ConfigConsumer.
    fn request_config<C: NodeComponents>(&self, provider: &impl ConfigProviderInterface<C>);
}

impl<T> ConfigConsumerProxy for &AsValue<T> {
    fn request_config<C: NodeComponents>(&self, _: &impl ConfigProviderInterface<C>) {}
}

impl<T: ConfigConsumer> ConfigConsumerProxy for AsValue<T> {
    fn request_config<C: NodeComponents>(&self, provider: &impl ConfigProviderInterface<C>) {
        provider.get::<T>();
    }
}

pub fn blackhole_socket<Req, Res>() -> Socket<Req, Res>
where
    Req: Send + 'static,
    Res: Send + 'static,
{
    struct Worker<Req, Res>(PhantomData<(Req, Res)>);
    unsafe impl<Req, Res> Sync for Worker<Req, Res>
    where
        Req: Send + 'static,
        Res: Send + 'static,
    {
    }
    impl<Req, Res> AsyncWorkerUnordered for Worker<Req, Res>
    where
        Req: Send + 'static,
        Res: Send + 'static,
    {
        type Request = Req;
        type Response = Res;
        fn handle(
            &self,
            _: Self::Request,
        ) -> impl futures::prelude::Future<Output = Self::Response> + Send {
            eprintln!(
                "Blackhole worker got a request of type {}",
                type_name::<Req>()
            );
            futures::future::pending()
        }
    }
    Worker::<Req, Res>(PhantomData).spawn()
}
