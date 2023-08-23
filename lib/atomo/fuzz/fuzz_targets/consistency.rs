#![no_main]

use std::iter::repeat;

use arbitrary::{Arbitrary, Unstructured};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: Input| {
    fuzz(data);
});

#[derive(Arbitrary, Debug)]
struct Input {
    #[arbitrary(with = |u: &mut Unstructured| u.int_in_range(0..=16))]
    num_query_threads: u8,
    #[arbitrary(with = |u: &mut Unstructured| u.int_in_range(1..=200))]
    num_run_queries: u8,
    #[arbitrary(with = |u: &mut Unstructured| u.int_in_range(1..=200))]
    num_run_updates: u8,
}

fn fuzz(input: Input) {
    let mut db =
        atomo::AtomoBuilder::<atomo::InMemoryStorage, atomo::DefaultSerdeBackend>::default()
            .with_table::<(), u64>("RUN")
            .with_table::<u64, u64>("TABLE_1")
            .with_table::<u64, u64>("TABLE_2")
            .build();

    let num_run_query_threads = input.num_query_threads as usize;
    let num_run_queries = input.num_run_queries as u64;
    let num_run_updates = input.num_run_updates as u64;

    let handles = repeat(db.query())
        .take(num_run_query_threads)
        .map(|query_runner| {
            std::thread::spawn(move || {
                for _ in 0..num_run_queries {
                    query_runner.run(|ctx| {
                        let mut table_1 = ctx.get_table::<u64, u64>("TABLE_1");
                        let mut table_2 = ctx.get_table::<u64, u64>("TABLE_2");

                        let Some(run) = ctx.get_table::<(), u64>("RUN")
                            .get(())
                            else {
                                for i in 0..100 {
                                    assert!(table_1.get(i).is_none());
                                    assert!(table_2.get(i).is_none());
                                }

                                return;
                            };

                        assert_eq!(table_1.get(run), Some(run + 12), "Run {run} failed");
                        assert_eq!(table_2.get(run), Some(run + 10), "Run {run} failed");

                        // remove from here.
                        table_1.remove(run);
                        table_2.remove(run);

                        // we should see the local change.
                        assert_eq!(table_1.get(run), None);
                        assert_eq!(table_2.get(run), None);
                    });
                }
            })
        })
        .collect::<Vec<_>>();

    for run in 0..num_run_updates {
        db.run(|ctx| {
            let mut run_table = ctx.get_table::<(), u64>("RUN");
            let mut table_1 = ctx.get_table::<u64, u64>("TABLE_1");
            let mut table_2 = ctx.get_table::<u64, u64>("TABLE_2");

            if run > 0 {
                let key = run - 1;
                assert_eq!(table_1.get(key).unwrap(), key + 12);
                assert_eq!(table_2.get(key).unwrap(), key + 10);
            }

            for _ in 0..7 {
                let prev_1 = table_1.get(run).unwrap_or(3 + run);
                let prev_2 = table_2.get(run).unwrap_or(5 + run);

                table_2.insert(run, prev_1 + 1);
                table_1.insert(run, prev_2 + 1);
            }

            assert_eq!(table_1.get(run).unwrap(), run + 12);
            assert_eq!(table_2.get(run).unwrap(), run + 10);

            run_table.insert((), run);
        });
    }

    handles
        .into_iter()
        .for_each(|handle| handle.join().expect("Running queries failed."));
}
