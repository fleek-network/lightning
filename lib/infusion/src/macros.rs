/// This is the main macro for this project. It can be used in 4 different mode of operation:
///
/// 1. Globally to define a `Collection` trait, and `Container` struct.
/// 2. In a trait to auto add the generics and default implementation of the required methods.
/// 3. In a triat to mark an object as an input.
/// 4. In an `impl` block to overwrite the methods.
///
/// After reading this documentation, refer to the example directory to see it in action.
///
/// # 1. Collections and Containers
///
/// Two core concepts in this library are `Collection` and [`Container`](crate::Container).
/// Container is exported by infusion as an struct. However `Collection` is project depended.
/// And should be generated using this macro.
///
/// There is no magic, a collection is simply a trait with a series of generics that have the same
/// name as the type name.
///
/// ```ignore
/// trait Collection {
///     type A: A<Collection = Self>;
///     type B: B<Collection = Self>;
///
///     fn build_graph() -> infusion::DependencyGraph {
///         // ...
///     }
/// }
/// ```
///
/// This way we can have the auto-implemented method `build_graph` that can always return the
/// dependency graph of the concrete types.
///
/// We can use this macro in this way to generate it:
///
/// ```ignore
/// infu!(@Collection [
///     A,
///     B
/// ]);
/// ```
///
/// This usage of the macro will make our `Collection` trait, notice how the name is not passed
/// that is because the name `Collection` is one place where infusion is opinionated and requires
/// it to be present and in scope for other use cases of the macro.
///
/// # 2. Inside a Trait
///
/// This use case is designed to be used inside of `trait` definition. It will add the methods
/// required to make the trait capable of being an infusion service.
///
/// ## Requirement
///
/// Using this macro assumes that an interface named `Collection`, and the container object
/// are present and imported with the names `Collection` and `Container`.
///
/// It also assumes that for trait `trait X {}` we have `Collection<X = X>`.
///
/// ## General Structure
///
/// The general structure of this macro is as follow:
///
/// ```ignore
/// trait MyTrait: Sized {
///     // Notice how the name `MyTrait` is repeated here. It is important for the name to
///     // be the same as the trait name.
///     infu!(MyTrait, {
///         // This function is called when we try to initialize the object and each dep
///         // is resolved and has the proper type from within the current collection.
///         //
///         // Parameters that are required by this function are considered our direct
///         // dependencies.
///         //
///         // During initialization of the entire collection infusion attempts to perform
///         // a topological ordering of dependencies graph and will fail if there are
///         // any cycles at this stage.
///         fn init(dep: MyOtherTrait) {
///             Ok(Self::init(dep))
///         }
///
///         // This function is called immediately after every object is initialized.
///         //
///         // At this point you can ask for every object. These *dependencies* are
///         // not considered initialization dependency and having cycles does not
///         // matter.
///         fn post(dep: MyOtherOtherTrait) {
///             self.provide(dep)
///         }
///     });
///
///     // Your trait generic methods.
///
///     fn init(dep: <Self::Collection as Collection>::MyOtherTrait) -> Result<Self, AnyStaticErrorType>;
///
///     fn provide(&mut self, other: <Self::Collection as Collection>::MyOtherOtherTrait);
/// }
/// ```
///
/// The post function is optional and can be emitted, however there are other scenarios that we
/// should cover.
///
/// ## Default Shorthand
///
/// A useful shorthand might be initializing an object using the `default()` method, in that case
/// you can use the following shorthand:
///
/// ```ignore
/// trait MyTrait: Sized + Default {
///     infu!(MyTrait @ Default);
/// }
/// ```
///
/// This will leave the post block empty. And requires no dependency to be injected.
///
/// # 3. Input Objects
///
/// Another common case is when a trait does not provide any constructor and leaves it as an
/// implementation detail, for these objects we can not perform initialization and we also can
/// not inject any dependency to these object.
///
/// We refer to these objects as inputs, other traits can accept these inputs as dependency.
///
/// An input must be provided during the construction of the collection using the
/// [`Container::with`](crate::Container::with) method.
///
/// ```ignore
/// trait MyConfiguration: Sized {
///     infu!(MyConfiguration @ Input);
/// }
///
/// fn usage() {
///     Collection::<_>::build()
///         .with(ConfigObjectImpl::load("./my_file.toml"))
///         .initialize();
/// }
/// ```
///
/// # 4. Implementation Overwrite
///
/// However not recommended, but the implementation can overwrite the dependencies, this is useful
/// for certain scenarios involving tests. You should keep in mind that cycles are still prohibited
/// and your overwrite should not introduce any cycles.
///
/// ```ignore
/// impl<C> MyTrait for MyObject<C> where C: Collection<MyTrait = Self> {
///     type Collection = C;
///
///     infu!(impl {
///         // Place your `init` overwrite here.
///         fn init() {}
///
///         // Place your `post` overwrite here.
///         fn post() {}
///     });
/// }
/// ```
#[macro_export]
macro_rules! infu {
    // Case 1: Handle creation of the collection.
    (@Collection [$($service:tt),* $(,)?]) => {
        pub trait Collection {
        $(
            type $service: $service<Collection = Self> + 'static;
         )*

            fn build_graph() -> $crate::DependencyGraph {
                let mut vtables = Vec::<$crate::vtable::VTable>::new();

            $(
                vtables.push({
                    fn init<T: $service + 'static>(
                        container: &$crate::Container
                    ) -> ::std::result::Result<
                        $crate::vtable::Object, ::std::boxed::Box<dyn std::error::Error>
                    > {
                        T::infu_initialize(container).map($crate::vtable::Object::new)
                    }

                    fn post<T: $service + 'static>(
                        obj: &mut $crate::vtable::Object,
                        container: &$crate::Container
                    ) {
                        let obj = obj.downcast_mut::<T>();
                        obj.infu_post_initialize(container);
                    }

                    $crate::vtable::VTable::new::<Self::$service>(
                        stringify!($service),
                        <Self::$service as $service>::infu_dependencies,
                        init::<Self::$service>,
                        post::<Self::$service>,
                    )
                });
             )*

                $crate::DependencyGraph::new(vtables)
            }
        }
    };

    // Case 2: Inside the trait.
    ($trait_name:tt, {
        fn init($($init_dep_name:ident: $init_dep_ty:tt),* $(,)?) $init:block

        fn post($($post_dep_name:ident: $post_dep_ty:tt),* $(,)?) $post:block
    }) => {
        /// The [`infusion`] collection that contains this trait.
        type Collection: Collection<$trait_name = Self>;

        infu!(impl {
            fn init($($init_dep_name: $init_dep_ty),*) $init
        });

        infu!(impl {
            fn post($($post_dep_name: $post_dep_ty),*) $post
        });
    };

    // Case 3: Input objects.

    // The `infu!(TraitName @ Input)` specifies that the current trait is an input trait and can
    // not be instantiated from within the system and it is required for a value to be provided
    // using the `.with` function before initialize is called.
    //
    // A use case for such an interface is the `ConfigProvider`.
    //
    // The return type for `infu_initialize` is set to
    ($trait_name:tt @ Input) => {
        type Collection: Collection<$trait_name = Self>;

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

        #[doc(hidden)]
        fn infu_post_initialize(&mut self, container: &$crate::Container) {
            // empty
        }
    };

    // Case 4: Implement the method.
    // 1. init: infu_dependencies, infu_initialize
    // 2. post: infu_post_initialize

    // Handle overwriting `init` in the implementation.
    (impl {
        fn init($($init_dep_name:ident: $init_dep_ty:tt),* $(,)?) $init:block
    }) => {
        #[doc(hidden)]
        fn infu_dependencies(visitor: &mut $crate::graph::DependencyGraphVisitor) {
        $(
            visitor.add_dependency(
                $crate::tag!(<Self::Collection as Collection>::$init_dep_ty as $init_dep_ty)
            );
         )*
        }

        #[doc(hidden)]
        fn infu_initialize(
            __container: &$crate::Container
        ) -> ::std::result::Result<Self, ::std::boxed::Box<dyn std::error::Error>> {
        $(
            let $init_dep_name = __container.get::<<Self::Collection as Collection>::$init_dep_ty>(
                $crate::tag!(<Self::Collection as Collection>::$init_dep_ty as $init_dep_ty)
            );
         )*

            // Make the container inaccessible to the block.
            let __container = ();

            let tmp: ::std::result::Result<Self, _> = {
                $init
            };

            tmp.map_err(|e| e.into())
        }
    };

    // Handle overwriting `post` in the implementation.
    (impl {
        fn post($($post_dep_name:ident: $post_dep_ty:tt),* $(,)?) $post:block
    }) => {
        #[doc(hidden)]
        fn infu_post_initialize(&mut self, __container: &$crate::Container) {
        $(
            let $post_dep_name = __container.get::<<Self::Collection as Collection>::$post_dep_ty>(
                $crate::tag!(<Self::Collection as Collection>::$post_dep_ty as $post_dep_ty)
            );
         )*

            { $post };
        }
    };

    // # Edge Case
    //
    // Below we handle the edge cases that don't really matter, these are done through recursive
    // expansion of normal cases.

    // Handle the case when `post` appears before `init` by rearranging them and doing a recursive
    // expansion.
    ($trait_name:tt, {
        fn post($($post_dep_name:ident: $post_dep_ty:tt),* $(,)?) $post:block

        fn init($($init_dep_name:ident: $init_dep_ty:tt),* $(,)?) $init:block
    }) => {
        infu!($trait_name, {
            fn init($($init_dep_name: $init_dep_ty),*) $init

            fn post($($post_dep_name: $post_dep_ty),*) $post
        });
    };

    // Handle the case when `post` is not provided by providing an empty version.
    ($trait_name:tt, {
        fn init($($init_dep_name:ident: $init_dep_ty:tt),* $(,)?) $init:block
    }) => {
        infu!($trait_name, {
            fn init($($init_dep_name: $init_dep_ty),*) $init

            fn post() {}
        });
    };

    // The `infu!(TraitName @ Default)` is a shorthand that will use the default function
    // to construct an instance of an item.
    ($trait_name:tt @ Default) => {
        infu!($trait_name, {
            fn init() {
                $crate::ok!(Self::default())
            }

            fn post() {}
        });
    };

    // Handle a single block for both `init` and `post`.
    (impl {
        fn init($($init_dep_name:ident: $init_dep_ty:tt),* $(,)?) $init:block

        fn post($($post_dep_name:ident: $post_dep_ty:tt),* $(,)?) $post:block
    }) => {
        infu!(impl {
            fn init($($init_dep_name: $init_dep_ty),*) $init
        });

        infu!(impl {
            fn post($($post_dep_name: $post_dep_ty),*) $post
        });
    };

    // Previous case but `post` comes before `init`.
    (impl {
        fn post($($post_dep_name:ident: $post_dep_ty:tt),* $(,)?) $post:block

        fn init($($init_dep_name:ident: $init_dep_ty:tt),* $(,)?) $init:block
    }) => {
        infu!(impl {
            fn init($($init_dep_name: $init_dep_ty),*) $init
        });

        infu!(impl {
            fn post($($post_dep_name: $post_dep_ty),*) $post
        });
    };
}

/// This macro can be used to shorthand common patterns of accessing a type on a collection.
/// Currently this macro is limited to a maximum depth of 2.
///
/// # Example
///
/// ```ignore
/// trait X {
///     infu!(...);
///
///     // fn init(
///     //     x: <Self::Collection as Collection>::X,
///     //     z: <<Self::Collection as Collection>::Y as Y>::Z,
///     // ) -> anyhow::Result<Self>;
///
///     fn init(
///         x: p!(::X)
///         z: p!(::Y::Z)
///     ) -> anyhow::Result<Self>;
/// }
/// ```
#[macro_export]
macro_rules! p {
    [:: $name:tt] => {
        <Self::Collection as Collection>::$name
    };

    [:: $name:tt < $($g:ty),* >] => {
        <Self::Collection as Collection>::$name<$($g),*>
    };

    [:: $name:tt :: $sub:ident] => {
        <<Self::Collection as Collection>::$name as $name>::$sub
    };

    [:: $name:tt :: $sub:ident < $($g:ty),* >] => {
        <<Self::Collection as Collection>::$name as $name>::$sub<$($g),*>
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
