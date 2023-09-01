#![no_main]

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: Input| {
    fuzz(data);
});

#[derive(Arbitrary, Debug)]
struct Input {
    operations: Vec<Op>,
}

#[derive(Arbitrary, Debug, Copy, Clone)]
enum Op {
    Insert { table: u8, key: u8, value: u64 },
    Remove { table: u8, key: u8 },
    Check { table: u8, key: u8 },
}

fn fuzz(input: Input) {
    let mut db =
        atomo::AtomoBuilder::<atomo::InMemoryStorage, atomo::DefaultSerdeBackend>::default()
            .with_table::<u8, u64>("TABLE_1")
            .with_table::<u8, u64>("TABLE_2")
            .with_table::<u8, u64>("TABLE_3")
            .with_table::<u8, u64>("TABLE_4")
            .with_table::<u8, u64>("TABLE_5")
            .build()
            .unwrap();

    let tables = (1..=5)
        .map(|i| db.resolve::<u8, u64>(&format!("TABLE_{i}")))
        .collect::<Vec<_>>();
    let result = db.run(move |ctx| {
        let mut reimp_tables = vec![fxhash::FxHashMap::<u8, u64>::default(); 5];

        let mut atomo_tables = tables.iter().map(|r| r.get(ctx)).collect::<Vec<_>>();

        for op in &input.operations {
            match op {
                Op::Check { table, key } => {
                    let tid = (table % 5) as usize;
                    let expected = reimp_tables[tid].get(key).copied();
                    let actual = atomo_tables[tid].get(key);
                    assert_eq!(expected, actual);
                },
                Op::Insert { table, key, value } => {
                    let tid = (table % 5) as usize;
                    reimp_tables[tid].insert(*key, *value);
                    atomo_tables[tid].insert(key, value);
                },
                Op::Remove { table, key } => {
                    let tid = (table % 5) as usize;
                    reimp_tables[tid].remove(key);
                    atomo_tables[tid].remove(key);
                },
            }
        }

        true
    });

    assert!(result);
}
