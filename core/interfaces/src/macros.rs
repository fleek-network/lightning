/// The macro to make a collection. This is only used internally in collection.rs. And is not meant
/// to be exported.
#[doc(hidden)]
#[macro_export]
macro_rules! collection {
    ([$($service:tt),* $(,)?]) => {
        pub trait Collection: Clone + Send + Sync + Sized + 'static {
        $(
            type $service: $service<Self> + 'static;
         )*

            fn build_graph() -> fdi::DependencyGraph {
                fdi::DependencyGraph::new()
                    .with_value($crate::_hacks::Blanket::default())
                    $(
                    .with_module::<<Self as Collection>::$service>()
                    )*
            }

            fn capture_configs(provider: &Self::ConfigProviderInterface);
        }
    }
}

/// This macro is useful for accessing members from a collection trait.
#[macro_export]
macro_rules! c {
    [$collection:tt :: $name:tt] => {
        <$collection as $crate::Collection>::$name
    };

    [$collection:tt :: $name:tt :: $sub:ident] => {
        <<$collection as $crate::Collection>::$name as $name<$collection>>::$sub
    };

    [$collection:tt :: $name:tt :: $sub:ident < $($g:ty),* >] => {
        <<$collection as $crate::Collection>::$name as $name<$collection>>::$sub<$($g),*>
    };
}

/// Generate a partial implementation of a collection this uses the provided types to assign the
/// associated types on the collection while filling every other member with the blanket.
///
/// This is a workaround on the fact that trait associated types do not have support for default
/// types (yet).
#[macro_export]
macro_rules! partial {
    (@gen_missing { $($name:ident),* }) => {
        $crate::proc::__gen_missing_assignments!({
            ConfigProviderInterface,
            KeystoreInterface,
            ApplicationInterface,
            BlockstoreInterface,
            BlockstoreServerInterface,
            SyncronizerInterface,
            BroadcastInterface,
            TopologyInterface,
            ArchiveInterface,
            ForwarderInterface,
            ConsensusInterface,
            HandshakeInterface,
            NotifierInterface,
            OriginProviderInterface,
            DeliveryAcknowledgmentAggregatorInterface,
            ReputationAggregatorInterface,
            ResolverInterface,
            RpcInterface,
            ServiceExecutorInterface,
            SignerInterface,
            FetcherInterface,
            PoolInterface,
            PingerInterface,
            IndexerInterface,
        }, { $($name),*});
    };
    (@gen_body { $($name:ident),* }) => {
        #[allow(unused_variables)]
        fn capture_configs(provider: &Self::ConfigProviderInterface) {
        }
    };
    ($struct:ident { $($name:ident = $ty:ty;)* }) => {
        #[derive(Clone)]
        pub struct $struct;

        impl $crate::Collection for $struct {
            $(type $name = $ty;)*
            $crate::partial!(@gen_missing { $($name),* });
            $crate::partial!(@gen_body { $($name),* });
        }
    };
    ($struct:ident require full { $($name:ident = $ty:ty;)* }) => {
        #[derive(Clone)]
        pub struct $struct;

        impl $crate::Collection for $struct {
            $(type $name = $ty;)*
            $crate::partial!(@gen_body { $($name),* });
        }
    };
}

#[cfg(test)]
mod tests {
    partial!(BlanketCollection {});

    // This test only has to be compiled in order to be considered passing.
    #[test]
    fn test_partial_no_missing_member() {
        fn expect_collection<C: crate::Collection>() {}
        expect_collection::<BlanketCollection>();
    }
}
