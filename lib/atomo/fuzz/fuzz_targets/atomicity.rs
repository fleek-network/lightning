#![no_main]

use arbitrary::{Arbitrary, Unstructured};
use atomo::{AtomoBuilder, DefaultSerdeBackend};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: Input| {
    fuzz(data);
});

#[derive(Arbitrary, Debug)]
struct Input {
    #[arbitrary(with = |u: &mut Unstructured| u.int_in_range(0..=16))]
    num_query_threads: u8,
    num_run_queries: u8,
    num_run_updates: u8,
}

fn fuzz(input: Input) {
    let num_run_query_threads = input.num_query_threads as usize;
    let num_run_queries = input.num_run_queries as u64;
    let num_run_updates = input.num_run_updates as usize;

    let mut db = AtomoBuilder::<DefaultSerdeBackend>::new()
        .with_table::<u8, usize>("TABLE")
        .build();

    let handles: Vec<_> = std::iter::repeat(db.query())
        .enumerate()
        .take(num_run_query_threads)
        .map(|(i, db)| {
            std::thread::Builder::new()
                .name(format!("QUERY {i}"))
                .spawn(move || {
                    let handle = db.resolve::<u8, usize>("TABLE");
                    for _ in 0..num_run_queries {
                        db.run(|ctx| {
                            let t = handle.get(ctx);
                            let a = t.get(0);
                            let b = t.get(1);
                            assert_eq!(a, b);
                        });
                    }
                })
                .expect("Failed to run thread")
        })
        .collect();

    let handle = db.resolve::<u8, usize>("TABLE");
    for i in 0..num_run_updates {
        db.run(|ctx| {
            let mut t = handle.get(ctx);
            t.insert(0, i);
            t.insert(1, i);
        });
    }

    handles
        .into_iter()
        .for_each(|handle| handle.join().expect("Running queries failed."));
}
