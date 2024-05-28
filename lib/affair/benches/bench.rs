use affair::{AsyncWorker, Socket, Worker};
use criterion::measurement::Measurement;
use criterion::*;

const ITER: u64 = 100_000;

fn bench(c: &mut Criterion) {
    let mut g = c.benchmark_group("Affair Worker");
    g.sample_size(20);

    bench_socket(&mut g, "tokio", || Worker::spawn(CounterWorker::default()));

    bench_socket(&mut g, "tokio-async", || {
        AsyncWorker::spawn(CounterWorker::default())
    });

    g.finish();
}

fn bench_socket<M, F>(g: &mut BenchmarkGroup<M>, name: &str, socket: F)
where
    M: Measurement,
    F: Fn() -> Socket<u64, u64>,
{
    g.throughput(Throughput::Elements(ITER));
    g.bench_function(BenchmarkId::new(name, "send"), |b| {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .thread_name(format!("{name}-send"))
            .enable_all()
            .build()
            .expect("Could not build the runtime");

        let socket = rt.block_on(async { socket() });

        b.to_async(rt).iter(|| async {
            for i in 0..ITER {
                let res = socket.run(i).await.expect("Failed");
                black_box(res);
            }
        });
    });

    g.throughput(Throughput::Elements(ITER));
    g.bench_function(BenchmarkId::new(name, "enqueue"), |b| {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .thread_name(format!("{name}-enqueue"))
            .enable_all()
            .build()
            .expect("Could not build the runtime");

        let socket = rt.block_on(async { socket() });

        b.to_async(rt).iter(|| async {
            for i in 0..ITER {
                let fut = socket.enqueue(i);
                black_box(fut).await.expect("Failed");
            }
        });
    });
}

#[derive(Default)]
struct CounterWorker {
    current: u64,
}

impl Worker for CounterWorker {
    type Request = u64;
    type Response = u64;

    fn handle(&mut self, req: Self::Request) -> Self::Response {
        self.current += req;
        self.current
    }
}

impl AsyncWorker for CounterWorker {
    type Request = u64;
    type Response = u64;

    async fn handle(&mut self, req: Self::Request) -> Self::Response {
        self.current += req;
        self.current
    }
}

criterion_group!(benches, bench);
criterion_main!(benches);
