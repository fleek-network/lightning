use std::marker::PhantomData;

use fdi::BuildGraph;

use crate::{Collection, ConfigConsumer, ConfigProviderInterface};

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

pub trait ConfigConsumerProxy {
    /// Request the config of Self if Self: ConfigConsumer.
    fn request_config<C: Collection>(provider: &C::ConfigProviderInterface);
}

impl<T> ConfigConsumerProxy for &AsValue<T> {
    fn request_config<C: Collection>(_: &C::ConfigProviderInterface) {}
}

impl<T: ConfigConsumer> ConfigConsumerProxy for AsValue<T> {
    fn request_config<C: Collection>(provider: &C::ConfigProviderInterface) {
        provider.get::<T>();
    }
}
