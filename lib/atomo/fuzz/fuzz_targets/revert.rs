#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: usize| {
    fuzz(data);
});


fn fuzz(n: usize) {
    let n = n % 10_000;

    let mut db = atomo::AtomoBuilder::<atomo::DefaultSerdeBackend>::default()
        .with_table::<u8, u64>("TABLE_1")
        .build();

    let table = db.resolve::<u8, u64>("TABLE_1");
    let query_runner = db.query();
    let handle = std::thread::spawn(move || {
        for _ in 0..n * 2 {
            query_runner.run(|ctx| {
                let mut table = table.get(ctx);
                assert_eq!(table.get(1), None);
                table.insert(1, 69);
                assert_eq!(table.get(1), Some(69));
            });
        }
    });

    for _ in 0..n {
        db.run(|ctx| {
            let mut table = table.get(ctx);
            assert_eq!(table.get(1), None);
            table.insert(1, 27);
            assert_eq!(table.get(1), Some(27));
            table.remove(1);
        });

    }

    handle.join().expect("Query thread failed");
}


