/// The macro to make a collection.
#[macro_export]
macro_rules! collection {
    ([$($service:tt),* $(,)?]) => {
        pub trait Collection: Clone + Send + Sync + Sized + 'static {
        $(
            type $service: $service<Self> + 'static;
         )*

            fn build_graph() -> fdi::DependencyGraph {
                DependencyGraph::new()
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

#[macro_export]
macro_rules! c {
    [$collection:tt :: $name:tt] => {
        <$collection as Collection>::$name
    };

    [$collection:tt :: $name:tt :: $sub:ident] => {
        <<$collection as Collection>::$name as $name<$collection>>::$sub
    };

    [$collection:tt :: $name:tt :: $sub:ident < $($g:ty),* >] => {
        <<$collection as Collection>::$name as $name<$collection>>::$sub<$($g),*>
    };
}
