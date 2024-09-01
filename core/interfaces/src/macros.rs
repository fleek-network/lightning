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

            /// Build the `fdi` dependency graph of this collection.
            fn build_graph() -> fdi::DependencyGraph {
                use $crate::prelude::*;

                let trace_shutdown = std::env::var("TRACE_SHUTDOWN").is_ok();
                let shutdown = $crate::ShutdownController::new(trace_shutdown);
                let waiter = shutdown.waiter();

                fdi::DependencyGraph::new()
                    .with_value(shutdown)
                    .with_value(waiter)
                    .with_value($crate::_hacks::Blanket::default())
                    .with_infallible(|this: &<Self as Collection>::ApplicationInterface|
                        ApplicationInterface::sync_query(this)
                    )
                    .with_infallible(|this: &<Self as Collection>::ReputationAggregatorInterface|
                        ReputationAggregatorInterface::get_reporter(this)
                    )
                    .with_infallible(|this: &<Self as Collection>::ReputationAggregatorInterface|
                        ReputationAggregatorInterface::get_query(this)
                    )
                    $(
                    .with_module::<<Self as Collection>::$service>()
                    )*
            }

            /// The implementation should call `provider.get::<ty>` for every member that
            /// implements ConfigConsumer.
            ///
            /// An implementation is provided when using the partial macro.
            fn capture_configs(provider: &impl $crate::ConfigProviderInterface<Self>);
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
            CheckpointerInterface,
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
            TaskBrokerInterface,
            SignerInterface,
            FetcherInterface,
            PoolInterface,
            PingerInterface,
            IndexerInterface,
        }, { $($name),*});
    };
    (@gen_body { $($name:ident = $ty:ty;)* }) => {

        // You might wanna read this article to understand what's going on here:
        // https://github.com/dtolnay/case-studies/blob/master/autoref-specialization/README.md
        // perm-commit-hash: b2c0e963b7c250587c1bdf80e7e7039b7c99d4c4
        #[allow(unused)]
        fn capture_configs(provider: &impl $crate::ConfigProviderInterface<Self>) {
            use $crate::_hacks::ConfigConsumerProxy;

            $(
            (&$crate::_hacks::AsValue::<$ty>::default()).request_config::<Self>(provider);
            )*
        }

    };
    ($struct:ident { $($name:ident = $ty:ty;)* }) => {
        #[derive(Clone)]
        pub struct $struct;

        impl $crate::Collection for $struct {
            $(type $name = $ty;)*
            $crate::partial!(@gen_missing { $($name),* });
            $crate::partial!(@gen_body { $($name = $ty;)* });
        }
    };
    // In this case we don't provide the missing types.
    ($struct:ident require full { $($name:ident = $ty:ty;)* }) => {
        #[derive(Clone)]
        pub struct $struct;

        impl $crate::Collection for $struct {
            $(type $name = $ty;)*
            $crate::partial!(@gen_body { $($name = $ty;)* });
        }
    };
}

/// A macro to spawn tokio tasks in the binary
/// takes the future you want to spawn as the first argument, a name for the tasks as the second
/// argument  If the task being spawned is crucial to the binary pass a shutdown waiter to it as an
/// optional third argument to ensure the node shutdowns if it panics
#[macro_export]
macro_rules! spawn {
    ($future:expr, $name:expr, crucial($waiter:expr)) => {
        tokio::task::Builder::new().name(&format!("{}#WAITER", $name)).spawn(async move{
            let handle = tokio::task::Builder::new().name($name).spawn($future).expect("Tokio task created outside of tokio runtime");

            if let Err(e) = handle.await {
                tracing::error!("Crucial task {} had a panic: {:?} \n Signaling to shutdown the rest of the node", $name, e);
                $crate::ShutdownWaiter::trigger_shutdown(&$waiter);
            }
        }).expect("Tokio task created outside of tokio runtime")
    };
    ($future:expr, $name:expr) => {
        tokio::task::Builder::new().name($name).spawn($future).expect("Tokio task created outside of tokio runtime");
    };
}

/// A macro to spawn tokio affair workers in the binary.
/// Takes an affair worker and spawns it, returning the socket.
/// A shutdown waiter is passed to as well which makes the worker shutdownable.
/// If the crucial flag is passed, a panic in the worker task will trigger a shutdown notification.
#[macro_export]
macro_rules! spawn_worker {
    ($worker:expr, $name:expr, $waiter:expr, crucial) => {{
        let (_, socket) = $worker.spawn_with_spawner(|fut| {
            let waiter_notify = $waiter.clone();
            tokio::task::Builder::new().name(&format!("{}#WAITER", $name)).spawn(async move {
                let shutdownable_fut = async move {
                    tokio::select! {
                        _ = fut => {},
                        _ = $waiter.wait_for_shutdown() => {}
                    }
                };
                let handle = tokio::task::Builder::new().name($name).spawn(shutdownable_fut).expect("Tokio task created outside of tokio runtime");
                if let Err(e) = handle.await {
                    tracing::error!("Crucial task {} had a panic: {:?} \n Signaling to shutdown the rest of the node", $name, e);
                    $crate::ShutdownWaiter::trigger_shutdown(&waiter_notify);
                }
            }).expect("Tokio task created outside of tokio runtime");
        });
        socket
    }};
    ($worker:expr, $name:expr, $waiter:expr) => {{
        let (_, socket) = $worker.spawn_with_spawner(|fut| {
            let waiter_notify = $waiter.clone();
            let shutdownable_fut = async move {
                tokio::select! {
                    _ = fut => {},
                    _ = $waiter.wait_for_shutdown() => {}
                }
            };
            tokio::task::Builder::new().name($name).spawn(shutdownable_fut).expect("Tokio task created outside of tokio runtime");
        });
        socket
    }};
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use affair::AsyncWorkerUnordered;
    partial!(BlanketCollection {});

    // This test only has to be compiled in order to be considered passing.
    #[test]
    fn test_partial_no_missing_member() {
        fn expect_collection<C: crate::Collection>() {}
        expect_collection::<BlanketCollection>();
    }

    #[test]
    fn test_spawn_macro_panic_shutdown() {
        // Ensure that a spawned task using our macro signals a shutdown when it panics
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                let shutdown_controller = crate::ShutdownController::new(false);
                let waiter = shutdown_controller.waiter();
                let panic_waiter = shutdown_controller.waiter();

                // Spawn a thread using the spawn macro that panics after 20ms
                spawn!(
                    async move {
                        tokio::time::sleep(Duration::from_millis(20)).await;
                        panic!();
                    },
                    "TEST",
                    crucial(panic_waiter)
                );

                // If the thread that panics doesnt trigger a shutdown afer it panics, fail the test
                if let Err(e) =
                    tokio::time::timeout(Duration::from_millis(100), waiter.wait_for_shutdown())
                        .await
                {
                    println!("{e}");
                    panic!("Failed to signal a shutdown when spawn thread panicked");
                }
            });
    }

    #[test]
    fn test_spawn_worker_macro_panic_shutdown() {
        // Ensure that a spawned task using our macro signals a shutdown when it panics
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                let shutdown_controller = crate::ShutdownController::new(false);
                let waiter = shutdown_controller.waiter();
                let panic_waiter = shutdown_controller.waiter();

                // Spawn a worker that will panic when we send a request
                let worker = TestWorker {};
                let socket = spawn_worker!(worker, "TEST-WORKER", panic_waiter, crucial);
                socket.enqueue(TestRequest {}).await.unwrap();

                // If the worker that panics doesn't trigger a shutdown afer it panics, fail the
                // test
                if let Err(e) =
                    tokio::time::timeout(Duration::from_millis(100), waiter.wait_for_shutdown())
                        .await
                {
                    println!("{e}");
                    panic!("Failed to signal a shutdown when worker panicked");
                }
            });
    }

    pub struct TestWorker {}

    #[derive(Clone, Debug)]
    pub struct TestRequest {}

    #[derive(Debug)]
    pub struct TestResponse {}

    impl AsyncWorkerUnordered for TestWorker {
        type Request = TestRequest;
        type Response = TestResponse;

        async fn handle(&self, _req: Self::Request) -> Self::Response {
            panic!();
        }
    }
}
