use std::sync::Arc;

use atomo::{AtomoBuilder, DefaultSerdeBackend, InMemoryStorage};

fn main() {
    type Key = u64;
    type Value = u64;

    // Create a database with the provided configuration and setup.
    // We should provide the different tables using the `with_table` method
    // and provide the type for the key and value pair in that table.
    //
    // It is important to always use the same type for accessing the key and value on
    // a table since Atomo perform runtime type confirmation to ensure that the same
    // rust type is used on the table when accessing the table.
    //
    // Once we're done with providing the tables we call `build` to build and
    // open the database.
    //
    // This ensure that only one update is happening at any given time.
    let mut db = AtomoBuilder::<InMemoryStorage, DefaultSerdeBackend>::default()
        .with_table::<Key, Value>("name-of-table")
        .build()
        .unwrap();

    // Build returns an `Atomo<UpdatePerm, _>` which allows us to mutate the data
    // there can only ever be one Atomo instance with update permission.
    //
    // Through Rust's borrow check logic we ensure that by explicitly making an
    // `Atomo<UpdatePerm, _>: ~Clone`.
    //
    // But we can have as many `Atomo<QueryPerm, _>` as we want.
    //
    // Here we get access to a query runner instance.
    let query_runner = db.query();

    // It is generally a good idea to cache the table resolution. This removes the need to perform
    // a table lookup and type check when accessing the table while running query/mutations.
    //
    // And since the type check happens during instantiating, you can avoid possible type
    // mismatches and therefore is safer to do so.
    let table_res = db.resolve::<Key, Value>("name-of-table");

    // Insert `(0, 17)` to the table.
    //
    // For the sake of example we are manually writing the type of the closures callback.
    // However this is not recommended to hard code your logic to a certain backend or encoding
    // therefore you should always instead prefer to use `ctx: _` as shown in the next call to
    // this function.
    db.run(
        |ctx: &mut atomo::TableSelector<InMemoryStorage, atomo::BincodeSerde>| {
            let mut table_ref = table_res.get(ctx);
            // Or if we didn't have a `ResolvedTableReference`.
            // let mut table_ref = ctx.get_table::<Key, Value>("name-of-table");

            table_ref.insert(0, 17);
        },
    );

    // Here we demonstrate the consistent views that Atomo provides:
    //
    // We create a thread that will be responsible for running a query. The current thread will try
    // to update a value in the middle of the execution of the query.
    //
    // We use two [`std::sync::Barrier`]s for synchronization.
    //
    // This is the linear log of the execution that we want to make happen:
    //
    // [Query Thread]: Enter the `run` closure. And print the value for key=0.
    // [Main Thread]:  Insert (key=0, value=12) to the table.
    // [Query Thread]: Read the value for key=0 again.
    //
    // The value when read the second time MUST equal to the initial read (17) since
    // the query is still running on the same snapshot of data.

    let barrier_1 = Arc::new(std::sync::Barrier::new(2));
    let barrier_2 = Arc::new(std::sync::Barrier::new(2));

    let c_1 = barrier_1.clone();
    let c_2 = barrier_2.clone();
    let handle = std::thread::spawn(move || {
        query_runner.run(|ctx: _| {
            let table_ref = table_res.get(ctx);
            println!("Query Started [get(0) == {:?}]", table_ref.get(0));

            // Allow the main thread to continue.
            c_1.wait();

            // Waiting until the main thread updates the data.
            c_2.wait();

            println!("Query Finishing [get(0) == {:?}]", table_ref.get(0));
        });

        // Run a second query this should get the new data.
        query_runner.run(|ctx: _| {
            let table_ref = table_res.get(ctx);
            println!("2nd Query Got [get(0) == {:?}]", table_ref.get(0));
        });
    });

    // Wait for the query thread to 'start' running the query.
    barrier_1.wait();

    println!("Starting the update");
    db.run(|ctx: _| {
        let mut table_ref = table_res.get(ctx);

        // Update the value and then allow the
        table_ref.insert(0, 12);
        println!("Value updated to 12");
    });

    // Allow the query thread to continue.
    barrier_2.wait();

    // Wait for the query thread to finish executing.
    let _ = handle.join();
}
