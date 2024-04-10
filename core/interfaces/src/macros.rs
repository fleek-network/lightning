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
        }

        $crate::proc::__gen_partial_macro!({
            $($service),*
        });
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
