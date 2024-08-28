# Merklize

The `merklize` crate builds on [atomo](../atomo) to provide a database-backed Merkle state tree, enabling efficiently verifiable state storage.

## Usage

```rust
use atomo::{
    AtomoBuilder,
    DefaultSerdeBackend,
    InMemoryStorage,
    SerdeBackend,
    StorageBackendConstructor,
};
use merklize::hashers::keccak::KeccakHasher;
use merklize::trees::mpt::MptStateTree;
use merklize::{StateProof, StateTree};

pub fn main() {
    let builder = InMemoryStorage::default();

    run::<_, MptStateTree<_, DefaultSerdeBackend, KeccakHasher>>(builder);
}

fn run<B: StorageBackendConstructor, T: StateTree<Storage = B::Storage>>(builder: B) {
    let mut db =
        T::register_tables(AtomoBuilder::new(builder).with_table::<String, String>("data"))
            .build()
            .unwrap();
    let query = db.query();

    // Open writer context and insert some data.
    db.run(|ctx| {
        let mut table = ctx.get_table::<String, String>("data");

        // Insert data.
        table.insert("key".to_string(), "value".to_string());

        // Update state tree.
        T::update_state_tree_from_context_changes(ctx).unwrap();
    });

    // Open reader context, read the data, get the state root hash, and get a proof of existence.
    query.run(|ctx| {
        let table = ctx.get_table::<String, String>("data");

        // Read the data.
        let value = table.get("key".to_string()).unwrap();
        println!("value: {:?}", value);

        // Get the state root hash.
        let state_root = T::get_state_root(ctx).unwrap();
        println!("state root: {:?}", state_root);

        // Get a proof of existence for some value in the state.
        let proof = T::get_state_proof(ctx, "data", T::Serde::serialize(&"key")).unwrap();
        println!("proof: {:?}", proof);

        // Verify the proof.
        proof
            .verify_membership::<String, String, T>(
                "data",
                "key".to_string(),
                "value".to_string(),
                state_root,
            )
            .unwrap();
    });
}
```

## Examples

There are a few examples in this codebase to help demonstrate usage. You can run them as follows:

```sh
cargo run --example mpt-keccak
cargo run --example mpt-blake3
cargo run --example jmt-sha256
cargo run --example not-merklized
```

## Tests

This crate contains a suite of unit tests that live along with the code. You can run them as follows:

```sh
cargo run test # or `cargo nextest run`
```

## Benchmarks

Run the benchmarks:

```sh
cargo bench
```

See [benches/README.md](./benches/README.md) for results.
