# Interface

This directory contains the high-level interfaces of the system, each interface is defined
using a `trait`. One thing to keep in mind is that interfaces can link to each other in a
generic way. And we use `type X: SomeOtherInterface` to indicate other generics. However
some interfaces are responsible to declare a concrete implementation of something.

For example the application layer that has a `SyncQueryRunner` is charge of dictating the
concrete type when implementing its interface. In other terms `SyncQueryRunner` is bounded
to the `Application`.

```rs
trait Application {
    // -- BOUNDED TYPES

    type SyncQuery: SyncQueryRunner;
}

trait SyncQueryRunner { }

// -- impl

struct App { }
struct SyncQuery { }

impl Aplpication for App {
    // Notice how `SyncQuery` here is a resolved type dictated by us.
    type SyncQuery = SyncQuery;
}
```

But in some other cases an interface takes as a generic input an implementer of some interface
for example `FileSystem` takes a `Blockstore`, it is important for a file system to work with
any underlying `Blockstore` so it **MUST NOT** dictate a concrete type when implementing.

```rs
trait Blockstore {}

trait FileSystem {
    // -- DYNAMIC TYPES

    type Blockstore: Blockstore;
}


// -- impl

struct Store { }

struct Fs { }

impl FileSystem for Fs {
    // ❌ This is wrong and bad. File system should not dictate the
    // type for dynamic generic types.
    type Blockstore = Store;
}

// -- 🟢 Right

struct Fs<B: Blockstore> { b: PhantomData<B> }

impl<B: Blockstore> FileSystem for Fs {
    // Notice how `B` here is not resolved and is generic.
    type Blockstore = B;
}
}
```

Currently we separate these two type of generic types in an interface via comment.

```rs
// -- DYNAMIC TYPES

// -- BOUNDED TYPES
```

