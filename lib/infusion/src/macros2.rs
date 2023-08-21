use paste::paste;

#[macro_export]
macro_rules! infu {
    // The `infu!(@ Input)` specifies that the current trait is an input trait and can
    // not be instantiated from within the system and it is required for a value to be provided
    // using the `.with` function before initialize is called.
    //
    // A use case for such an interface is the `ConfigProvider`.
    //
    // The return type for `infu_initialize` is set to
    (@ Input) => {
        #[doc(hidden)]
        fn infu_dependencies(visitor: &mut $crate::graph::DependencyGraphVisitor) {
            visitor.mark_input();
        }

        #[doc(hidden)]
        fn infu_initialize(
            _container: &$crate::Container
        ) -> ::std::result::Result<
            Self, ::std::boxed::Box<dyn std::error::Error>> where Self: Sized {
            unreachable!("This trait is marked as an input.")
        }
    };

    // The `infu!(@ Default)` specifies that the current implementation should be using the default
    // provided by the [`Default`] trait for initialization.
    (@ Default) => {
        #[doc(hidden)]
        fn infu_dependencies(__visitor: &mut $crate::graph::DependencyGraphVisitor) {}

        #[doc(hidden)]
        fn infu_initialize(
            __container: &$crate::Container
        ) -> ::std::result::Result<
            Self, ::std::boxed::Box<dyn std::error::Error>> where Self: Sized {
            $crate::ok!(Self::default())
        }
    };

    (impl<$collection_name:tt> {
        fn init($($dep_name:ident: $dep_ty:tt),* $(,)?) $block:block
    }) => {
        #[doc(hidden)]
        fn infu_dependencies(visitor: &mut $crate::graph::DependencyGraphVisitor) {
        $(
            visitor.add_dependency(
                $crate::tag!($collection_name :: $dep_ty)
            );
         )*
        }

        #[doc(hidden)]
        fn infu_initialize(
            __container: &$crate::Container
        ) -> ::std::result::Result<
            Self, ::std::boxed::Box<dyn std::error::Error>> where Self: Sized {
        $(
            let $dep_name: <$collection_name as Collection>::$dep_ty = __container.get(
                $crate::tag!($collection_name :: $dep_ty)
            );
         )*

            // Make the container inaccessible to the block.
            let __container = ();

            let tmp: ::std::result::Result<Self, _> = {
                $block
            };

            tmp.map_err(|e| e.into())
        }
    };

    (impl<$collection_name:tt> {
        fn post($($dep_name:ident: $dep_ty:tt),* $(,)?) $block:block
    }) => {
        #[doc(hidden)]
        fn infu_post_initialize(&mut self, container: &$crate::Container) {
        $(
            let $dep_name: <$collection_name as Collection>::$dep_ty = __container.get(
                $crate::tag!($collection_name :: $dep_ty)
            );
         )*

            // Make the container inaccessible to the block.
            let __container = ();

            {
                $block
            };
        }
    };

    // Handle a single block for both `init` and `post`.
    (impl<$collection_name:tt> {
        fn init($($init_dep_name:ident: $init_dep_ty:tt),* $(,)?) $init:block

        fn post($($post_dep_name:ident: $post_dep_ty:tt),* $(,)?) $post:block
    }) => {
        infu!(impl<$collection_name> {
            fn init($($init_dep_name: $init_dep_ty),*) $init
        });

        infu!(impl<$collection_name> {
            fn post($($post_dep_name: $post_dep_ty),*) $post
        });
    };

    (impl<$collection_name:tt> {
        fn post($($post_dep_name:ident: $post_dep_ty:tt),* $(,)?) $post:block

        fn init($($init_dep_name:ident: $init_dep_ty:tt),* $(,)?) $init:block
    }) => {
        infu!(impl<$collection_name> {
            fn init($($init_dep_name: $init_dep_ty),*) $init
        });

        infu!(impl<$collection_name> {
            fn post($($post_dep_name: $post_dep_ty),*) $post
        });
    };
}

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

/// Use this macro to generate a tag for a type.
#[macro_export]
macro_rules! tag {
    ($type:ty as $trait_name:tt) => {
        $crate::vtable::Tag::new::<$type>(
            stringify!($trait_name),
            <$type as $trait_name>::infu_dependencies,
        )
    };

    ($collection:tt :: $type:tt) => {
        $crate::vtable::Tag::new::<<$collection as Collection>::$type>(
            stringify!($type),
            <<$collection as Collection>::$type as $type<$collection>>::infu_dependencies,
        )
    };
}

/// Use this macro to generate the return type from an infallible init
/// function. This is when you never return an error and the type for
/// error is not available.
#[macro_export]
macro_rules! ok {
    ($e:expr) => {
        ::std::result::Result::<_, $crate::error::Infallible>::Ok($e)
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
