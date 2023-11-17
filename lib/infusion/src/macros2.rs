/// The macro to make a collection.
#[macro_export]
macro_rules! collection {
    // Case 1: Handle creation of the collection.
    ([$($service:tt),* $(,)?]) => {
        pub trait CollectionBase: Clone + Send + Sync + Sized + 'static {
        $(
            type $service<C: Collection>: $service<C> + 'static;
         )*
        }

        impl<T: CollectionBase> Collection for T {
        $(
            type $service = T::$service<Self>;
         )*
        }

        $crate::paste! {
        $(
        pub trait [ < $service Container > ]: Send + Sync + 'static {
            type $service<C: Collection>: $service<C> + 'static;
        }

        pub struct [ < $service Modifier > ]<
                N: [ < $service Container > ],
                O: CollectionBase
        >(core::marker::PhantomData<(N, O)>);

        impl<N: [ < $service Container > ], O: CollectionBase> Clone
                    for [ < $service Modifier > ]<N, O> {
            fn clone(&self) -> Self {
                Self(Default::default())
            }
        }
         )*
        }

        $crate::__modifier_helper!({$($service),*});

        pub trait Collection: Clone + Send + Sync + Sized + 'static {
        $(
            type $service: $service<Self> + 'static;
         )*

            fn build_graph() -> $crate::DependencyGraph {
                let mut vtables = Vec::<$crate::vtable::VTable>::new();

            $(
                vtables.push({
                    fn init<C: Collection, T: $service<C> + 'static>(
                        container: &$crate::Container
                    ) -> ::std::result::Result<
                        $crate::vtable::Object, ::std::boxed::Box<dyn std::error::Error>
                    > {
                        T::infu_initialize(container).map($crate::vtable::Object::new)
                    }

                    fn post<C: Collection, T: $service<C> + 'static>(
                        obj: &mut $crate::vtable::Object,
                        container: &$crate::Container
                    ) {
                        let obj = obj.downcast_mut::<T>();
                        obj.infu_post_initialize(container);
                    }

                    $crate::vtable::VTable::new::<Self::$service>(
                        stringify!($service),
                        <Self::$service as $service<Self>>::infu_dependencies,
                        init::<Self, Self::$service>,
                        post::<Self, Self::$service>,
                    )
                });
             )*

                $crate::DependencyGraph::new(vtables)
            }
        }

        #[derive(Clone)]
        pub struct BlankBinding;

        impl CollectionBase for BlankBinding {
        $(
            type $service<C: Collection> = $crate::Blank<C>;
         )*
        }

        infusion::__gen_macros_helper!({$($service),*});
    };

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
