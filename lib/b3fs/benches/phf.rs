use criterion::*;
use measurement::Measurement;
use rand::rngs::SmallRng;
use rand::seq::SliceRandom;
use rand::SeedableRng;

const DATA_1000_WORDS: &[u8] = include_bytes!("./assets/1000_words.txt");
const DATA_ALL_TARGET: &[u8] = include_bytes!("./assets/all_target.txt");
const DATA_LIGHTNING: &[u8] = include_bytes!("./assets/lightning.txt");

fn bench_phf(c: &mut Criterion) {
    let s1 = DATA_1000_WORDS.split(|b| *b == b'\n').collect::<Vec<_>>();
    let s2 = DATA_ALL_TARGET.split(|b| *b == b'\n').collect::<Vec<_>>();
    let s3 = DATA_LIGHTNING.split(|b| *b == b'\n').collect::<Vec<_>>();

    let mut g = c.benchmark_group("PHF");
    g.sample_size(10);

    for sz in [
        1,
        8,
        16,
        32,
        64,
        250,
        500,
        1000,
        2000,
        3000,
        5000,
        8000,
        10000,
        30000,
        45000,
        u16::MAX,
    ] {
        phf_run_bench("words", &mut g, sz as usize, &s1);
        phf_run_bench("all_target", &mut g, sz as usize, &s2);
        phf_run_bench("lightning", &mut g, sz as usize, &s3);
    }

    g.finish();
}

fn phf_run_bench<M: Measurement>(
    label: &str,
    g: &mut BenchmarkGroup<M>,
    size: usize,
    words: &[&[u8]],
) {
    if size > words.len() {
        return;
    }

    let mut rng = SmallRng::seed_from_u64(13224221);
    let mut words = words
        .choose_multiple(&mut rng, size)
        .copied()
        .collect::<Vec<_>>();
    words.sort_unstable();

    g.bench_with_input(BenchmarkId::new(label, size), &words, |b, words| {
        b.iter(|| {
            let mut x = b3fs::bucket::dir::phf::PhfGenerator::new(size);
            for (i, &w) in words.iter().enumerate() {
                let pos = (i as u32 + 1) * 40;
                x.push(w, pos);
            }
            let output = x.finalize();
            black_box(output);
        })
    });
}

criterion_group!(benches, bench_phf);
criterion_main!(benches);
