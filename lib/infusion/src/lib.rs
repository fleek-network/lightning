#![allow(clippy::all, unused)]

use std::{
    any::{Any, TypeId},
    marker::PhantomData,
};

#[derive(Default)]
pub struct DependencyCollector {}

impl DependencyCollector {
    pub fn depend<T: Any>(&self) {
        let _id = TypeId::of::<T>();
        todo!();
    }
}

macro_rules! c {
    [:: $name:tt] => {
        <Self::Collection as Collection>::$name
    };

    [:: $name:tt :: $sub:ident] => {
        <<Self::Collection as Collection>::$name as $name>::$sub
    };
}

macro_rules! infu_dep {
    [$($name:tt),*] => {
        #[doc(hidden)]
        fn infu_dependencies(collector: &mut DependencyCollector) {
            $(
                collector.depend::<
                    <Self::Collection as Collection>::$name
                >();
             );*
        }
    }
}

// ------------- App

pub trait AppInterface: Sized {
    type Collection: Collection<AppInterface = Self>;

    type SyncQueryRunner: SyncQueryRunnerInterface;

    fn get_sync_query_runner(&self) -> Self::SyncQueryRunner;

    #[doc(hidden)]
    fn infu_dependencies(_collector: &mut DependencyCollector) {
        // empty
    }

    #[doc(hidden)]
    fn infu_initialize(_container: &Container<Self::Collection>) -> Result<Self, ()> {
        todo!()
    }
}

pub trait SyncQueryRunnerInterface {
    fn get_len(&self) -> usize;
}

// ------------- Consensus

macro_rules! infu {
    [
        $name:ident ;

        fn init($($dep_name:ident: $dep_ty:tt),*) $init:block
    ] => {
        type Collection: Collection<$name = Self>;

        #[doc(hidden)]
        fn infu_dependencies(collector: &mut DependencyCollector) {
            $(
                collector.depend::<
                    <Self::Collection as Collection>::$dep_ty
                >();
             );*
        }

        #[doc(hidden)]
        fn infu_initialize(container: &Container<Self::Collection>) -> Result<Self, ()> {
            $(
                let $dep_name = container.get::<<Self::Collection as Collection>::$dep_ty>();
             );*

            $init
        }
    };
}

pub trait ConsensusInterface: Sized {
    infu![
        ConsensusInterface;
        fn init(app: AppInterface) {
            let sqr = app.get_sync_query_runner();
            Self::init(sqr)
        }
        // fn post_init(container) { self.provide(container.get_app().get_some_socket()) }
    ];

    fn init(
        // sqr: <<Self::Collection as Collection>::AppInterface as AppInterface>::SyncQueryRunner,
        sqr: c![::AppInterface::SyncQueryRunner],
    ) -> Result<Self, ()>;

    // #[doc(hidden)]
    // fn infu_dependencies(collector: &mut DependencyCollector) {
    //     collector.depend::<c![::AppInterface]>();
    // }
    //
    // #[doc(hidden)]
    // fn infu_initialize(container: &Container<Self::Collection>) -> Result<Self, ()> {
    //     let app = container.get_app();
    //     let sqr = app.get_sync_query_runner();
    //     Self::init(sqr)
    // }
}

// ------------- Container
// container! Container: ContainerInterface {
//      app: AppInterface,
//      consensus: ConsensusInterface,
// }
//
// all this should be generated from the above macro.

pub trait Collection {
    type AppInterface: AppInterface<Collection = Self> + Sized + 'static;
    type ConsensusInterface: ConsensusInterface<Collection = Self> + Sized + 'static;
}

pub struct Container<C: Collection> {
    app: Option<C::AppInterface>,
    consensus: Option<C::ConsensusInterface>,
}

impl<C: Collection> Container<C> {
    pub fn build() -> Self {
        let _dependencies: Vec<(usize, usize)> = Vec::new();

        let _tid = vec![
            TypeId::of::<C::AppInterface>(),
            TypeId::of::<C::ConsensusInterface>(),
        ];

        //
        let mut i = 0;

        let mut collector = DependencyCollector::default();
        <C::AppInterface as AppInterface>::infu_dependencies(&mut collector);
        i += 1;

        // more

        // topo sort the graph. report cycles.

        todo!()
    }

    pub fn initialize(self) {
        // based on the topo order
        todo!()
    }

    pub fn get<T: Sized + 'static>(&self) -> &T {
        todo!()
    }

    pub fn with_app(mut self, app: C::AppInterface) -> Self {
        self.app = Some(app);
        self
    }

    pub fn get_app(&self) -> &C::AppInterface {
        self.app.as_ref().unwrap()
    }

    pub fn with_consensus(mut self, consensus: C::ConsensusInterface) -> Self {
        self.consensus = Some(consensus);
        self
    }

    pub fn get_consensus(&self) -> &C::ConsensusInterface {
        self.consensus.as_ref().unwrap()
    }
}

// impl

struct App<C: Collection>(PhantomData<C>);
struct Sqr;
struct Consensus<C: Collection>(<C::AppInterface as AppInterface>::SyncQueryRunner);

impl<C> AppInterface for App<C>
where
    C: Collection<AppInterface = Self>,
{
    type Collection = C;

    type SyncQueryRunner = Sqr;

    fn get_sync_query_runner(&self) -> Self::SyncQueryRunner {
        Sqr
    }
}

impl SyncQueryRunnerInterface for Sqr {
    fn get_len(&self) -> usize {
        0
    }
}

impl<C> ConsensusInterface for Consensus<C>
where
    C: Collection<ConsensusInterface = Self>,
{
    type Collection = C;

    fn init(sqr: c![::AppInterface::SyncQueryRunner]) -> Result<Self, ()> {
        Ok(Consensus(sqr))
    }
}

struct ProdNode;

impl Collection for ProdNode {
    type AppInterface = App<ProdNode>;
    type ConsensusInterface = Consensus<ProdNode>;
}

fn x() {}
