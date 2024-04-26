use std::iter::repeat;
use std::task::Context;
use std::thread;
use std::time::{Duration, Instant};

use criterion::measurement::Measurement;
use criterion::*;
use futures::FutureExt;
use lightning_interfaces::ShutdownController;
use triomphe::Arc;

fn bench_over_many_threads<M: Measurement<Value = Duration>, S, R, O>(
    b: &mut Bencher<'_, M>,
    num_threads: u8,
    state: S,
    routine: R,
) where
    R: 'static + Clone + Copy + Send + Fn(&S) -> O,
    S: 'static + Send + Sync,
{
    let state_arc = Arc::new(state);
    b.iter_custom(|iters| {
        let thread_handles = repeat(routine)
            .map(|r| (state_arc.clone(), r))
            .take(num_threads as usize)
            .map(move |(state, routine)| {
                thread::spawn(move || {
                    let started = Instant::now();
                    for _ in 0..iters {
                        routine(&state);
                    }
                    started.elapsed()
                })
            })
            .collect::<Vec<_>>();

        let total_duration = thread_handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .reduce(|a, b| a + b)
            .unwrap();

        Duration::from_nanos((total_duration.as_nanos() / (num_threads as u128)) as u64)
    });
}

fn bench(c: &mut Criterion) {
    let mut g = c.benchmark_group("Shutdown");
    g.sample_size(1000);

    for n in [1, 2, 4, 8, 16, 24, 32] {
        g.bench_with_input(
            BenchmarkId::new("wait_for_shutdown+drop", n),
            &n,
            |b, num_threads| {
                let ctrl = ShutdownController::new();

                bench_over_many_threads(b, *num_threads, ctrl, |ctrl| {
                    let waiter = ctrl.waiter();
                    let future = waiter.wait_for_shutdown();
                    let future = black_box(future);
                    drop(future);
                    drop(black_box(waiter));
                });
            },
        );

        g.bench_with_input(
            BenchmarkId::new("wait_for_shutdown+poll+drop", n),
            &n,
            |b, num_threads| {
                let ctrl = ShutdownController::new();

                bench_over_many_threads(b, *num_threads, ctrl, |ctrl| {
                    let waiter = ctrl.waiter();
                    let future = waiter.wait_for_shutdown();
                    let mut future = Box::pin(black_box(future));
                    let poll =
                        future.poll_unpin(&mut Context::from_waker(&dummy_waker::dummy_waker()));
                    let _ = black_box(poll);
                    drop(future);
                    drop(black_box(waiter));
                });
            },
        );

        g.bench_with_input(
            BenchmarkId::new("wait_for_shutdown+poll+drop[same_waiter]", n),
            &n,
            |b, num_threads| {
                let ctrl = ShutdownController::new();

                bench_over_many_threads(b, *num_threads, ctrl.waiter(), |waiter| {
                    let future = waiter.wait_for_shutdown();
                    let mut future = Box::pin(black_box(future));
                    let poll =
                        future.poll_unpin(&mut Context::from_waker(&dummy_waker::dummy_waker()));
                    let _ = black_box(poll);
                    drop(future);
                });
            },
        );

        g.bench_with_input(
            BenchmarkId::new("wait_for_shutdown+poll+drop[post-shutdown]", n),
            &n,
            |b, num_threads| {
                let ctrl = ShutdownController::new();
                ctrl.trigger_shutdown();

                bench_over_many_threads(b, *num_threads, ctrl, |ctrl| {
                    let waiter = ctrl.waiter();
                    let future = waiter.wait_for_shutdown();
                    let mut future = Box::pin(black_box(future));
                    let poll =
                        future.poll_unpin(&mut Context::from_waker(&dummy_waker::dummy_waker()));
                    let _ = black_box(poll);
                    drop(future);
                    drop(black_box(waiter));
                });
            },
        );
    }

    g.finish();
}

criterion_group!(benches, bench);
criterion_main!(benches);
