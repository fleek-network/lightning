use atomo::{AtomoBuilder, DefaultSerdeBackend, InMemoryStorage};

pub fn main() {
    let builder = InMemoryStorage::default();

    let mut db = AtomoBuilder::<_, DefaultSerdeBackend>::new(builder)
        .with_table::<String, String>("data")
        .build()
        .unwrap();

    // Open writer context and insert some data.
    db.run(|ctx| {
        let mut table = ctx.get_table::<String, String>("data");

        // Insert data.
        table.insert("key".to_string(), "value".to_string());
    });

    // Open reader context and read the data.
    db.query().run(|ctx| {
        let table = ctx.get_table::<String, String>("data");

        // Read the data.
        let value = table.get("key".to_string()).unwrap();
        println!("value: {:?}", value);
    });
}
