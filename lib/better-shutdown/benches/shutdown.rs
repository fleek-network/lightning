use std::iter::repeat;
use std::task::Context;
use std::thread;
use std::time::{Duration, Instant};

use better_shutdown::ShutdownController;
use criterion::measurement::Measurement;
use criterion::*;
use futures::FutureExt;
use triomphe::Arc;

fn bench_over_many_threads<M: Measurement<Value = Duration>, S, R, O, F>(
    b: &mut Bencher<'_, M>,
    num_threads: u8,
    state: F,
    routine: R,
) where
    R: 'static + Clone + Copy + Send + Fn(&S, u8) -> O,
    S: 'static + Send + Sync,
    F: Fn() -> S,
{
    b.iter_custom(|iters| {
        let thread_handles = repeat(routine)
            .map(|r| (state(), r))
            .take(num_threads as usize)
            .enumerate()
            .map(move |(n, (state, routine))| {
                thread::spawn(move || {
                    let started = Instant::now();
                    for _ in 0..iters {
                        routine(&state, n as u8);
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
                let ctrl = Arc::new(ShutdownController::new(false));

                bench_over_many_threads(
                    b,
                    *num_threads,
                    || ctrl.clone(),
                    |ctrl, _| {
                        let waiter = ctrl.waiter();
                        let future = waiter.wait_for_shutdown();
                        let future = black_box(future);
                        drop(future);
                        drop(black_box(waiter));
                    },
                );
            },
        );

        g.bench_with_input(
            BenchmarkId::new("wait_for_shutdown+poll+drop", n),
            &n,
            |b, num_threads| {
                let ctrl = Arc::new(ShutdownController::new(false));

                bench_over_many_threads(
                    b,
                    *num_threads,
                    || ctrl.clone(),
                    |ctrl, _| {
                        let waiter = ctrl.waiter();
                        let future = waiter.wait_for_shutdown();
                        let mut future = Box::pin(black_box(future));
                        let poll = future
                            .poll_unpin(&mut Context::from_waker(&dummy_waker::dummy_waker()));
                        let _ = black_box(poll);
                        drop(future);
                        drop(black_box(waiter));
                    },
                );
            },
        );

        g.bench_with_input(
            BenchmarkId::new("wait_for_shutdown+poll2x+drop", n),
            &n,
            |b, num_threads| {
                let ctrl = Arc::new(ShutdownController::new(false));

                bench_over_many_threads(
                    b,
                    *num_threads,
                    || ctrl.clone(),
                    |ctrl, _| {
                        let waiter = ctrl.waiter();
                        let future = waiter.wait_for_shutdown();
                        let mut future = Box::pin(black_box(future));
                        let poll = future
                            .poll_unpin(&mut Context::from_waker(&dummy_waker::dummy_waker()));
                        let _ = black_box(poll);
                        let poll = future
                            .poll_unpin(&mut Context::from_waker(&dummy_waker::dummy_waker()));
                        let _ = black_box(poll);
                        drop(future);
                        drop(black_box(waiter));
                    },
                );
            },
        );

        g.bench_with_input(
            BenchmarkId::new("wait_for_shutdown+poll+drop[same_waiter]", n),
            &n,
            |b, num_threads| {
                let ctrl = ShutdownController::new(false);

                bench_over_many_threads(
                    b,
                    *num_threads,
                    || ctrl.waiter(),
                    |waiter, _| {
                        let future = waiter.wait_for_shutdown();
                        let mut future = Box::pin(black_box(future));
                        let poll = future
                            .poll_unpin(&mut Context::from_waker(&dummy_waker::dummy_waker()));
                        let _ = black_box(poll);
                        drop(future);
                    },
                );
            },
        );

        g.bench_with_input(
            BenchmarkId::new("wait_for_shutdown+poll+drop[busy_waiter]", n),
            &n,
            |b, num_threads| {
                let ctrl = ShutdownController::new(false);

                bench_over_many_threads(
                    b,
                    *num_threads,
                    || {
                        let mut waiter = ctrl.waiter();
                        waiter.mark_busy();
                        waiter
                    },
                    |waiter, _| {
                        let future = waiter.wait_for_shutdown();
                        let mut future = Box::pin(black_box(future));
                        let poll = future
                            .poll_unpin(&mut Context::from_waker(&dummy_waker::dummy_waker()));
                        let _ = black_box(poll);
                        drop(future);
                    },
                );
            },
        );

        g.bench_with_input(
            BenchmarkId::new("wait_for_shutdown+poll+drop[post-shutdown]", n),
            &n,
            |b, num_threads| {
                let ctrl = Arc::new(ShutdownController::new(false));
                ctrl.trigger_shutdown();

                bench_over_many_threads(
                    b,
                    *num_threads,
                    || ctrl.clone(),
                    |ctrl, _| {
                        let waiter = ctrl.waiter();
                        let future = waiter.wait_for_shutdown();
                        let mut future = Box::pin(black_box(future));
                        let poll = future
                            .poll_unpin(&mut Context::from_waker(&dummy_waker::dummy_waker()));
                        let _ = black_box(poll);
                        drop(future);
                        drop(black_box(waiter));
                    },
                );
            },
        );
    }

    g.finish();
}

criterion_group!(benches, bench);
criterion_main!(benches);
