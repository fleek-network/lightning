use std::iter::repeat;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll, Waker};
use std::time::Duration;

use better_shutdown::ShutdownController;
use futures::task::ArcWake;
use futures::FutureExt;

struct WakeCounter(AtomicUsize);

impl WakeCounter {
    pub fn get(&self) -> usize {
        self.0.load(Ordering::Relaxed)
    }
}

impl ArcWake for WakeCounter {
    fn wake_by_ref(arc_self: &std::sync::Arc<Self>) {
        arc_self.0.fetch_add(1, Ordering::SeqCst);
    }
}

fn new_waker() -> (Arc<WakeCounter>, Waker) {
    let counter = Arc::new(WakeCounter(AtomicUsize::new(0)));
    let waker = futures::task::waker(counter.clone());
    (counter, waker)
}

#[test]
fn test_01() {
    let ctrl = ShutdownController::new(false);
    ctrl.trigger_shutdown();
    let (counter, waker) = new_waker();
    let result = ctrl
        .wait_for_completion()
        .poll_unpin(&mut Context::from_waker(&waker));
    assert_eq!(counter.get(), 0);
    assert_eq!(result, Poll::Ready(()));
}

#[test]
fn test_02() {
    let ctrl = ShutdownController::new(false);
    let (counter, waker) = new_waker();
    let mut future = ctrl.wait_for_completion();
    let result = future.poll_unpin(&mut Context::from_waker(&waker));
    assert_eq!(counter.get(), 0);
    assert_eq!(result, Poll::Pending);
    ctrl.trigger_shutdown();
    let result = future.poll_unpin(&mut Context::from_waker(&waker));
    assert_eq!(counter.get(), 1);
    assert_eq!(result, Poll::Ready(()));
}

#[test]
fn test_03() {
    let ctrl = ShutdownController::new(false);
    let (counter, waker) = new_waker();
    let mut future = ctrl.wait_for_completion();
    let result = future.poll_unpin(&mut Context::from_waker(&waker));
    assert_eq!(counter.get(), 0);
    assert_eq!(result, Poll::Pending);
    let result = future.poll_unpin(&mut Context::from_waker(&waker));
    assert_eq!(counter.get(), 0);
    assert_eq!(result, Poll::Pending);
    ctrl.trigger_shutdown();
    let result = future.poll_unpin(&mut Context::from_waker(&waker));
    assert_eq!(counter.get(), 1);
    assert_eq!(result, Poll::Ready(()));
}

// multiple wake ups should not wake up more than once.
#[test]
fn test_04() {
    let ctrl = ShutdownController::new(false);
    let (counter, waker) = new_waker();
    let mut future = ctrl.wait_for_completion();
    let result = future.poll_unpin(&mut Context::from_waker(&waker));
    assert_eq!(counter.get(), 0);
    assert_eq!(result, Poll::Pending);
    ctrl.trigger_shutdown();
    ctrl.trigger_shutdown();
    let result = future.poll_unpin(&mut Context::from_waker(&waker));
    assert_eq!(counter.get(), 1);
    assert_eq!(result, Poll::Ready(()));
}

// poll after wake up should always be ready
#[test]
fn test_05() {
    let ctrl = ShutdownController::new(false);
    let (counter, waker) = new_waker();
    let mut future = ctrl.wait_for_completion();
    let result = future.poll_unpin(&mut Context::from_waker(&waker));
    assert_eq!(counter.get(), 0);
    assert_eq!(result, Poll::Pending);
    ctrl.trigger_shutdown();
    let result = future.poll_unpin(&mut Context::from_waker(&waker));
    assert_eq!(counter.get(), 1);
    assert_eq!(result, Poll::Ready(()));
    ctrl.trigger_shutdown();
    let result = future.poll_unpin(&mut Context::from_waker(&waker));
    assert_eq!(counter.get(), 1);
    assert_eq!(result, Poll::Ready(()));
}

// same waker multiple future should wake up multiple times.
#[test]
fn test_06() {
    let ctrl = ShutdownController::new(false);
    let (counter, waker) = new_waker();
    let mut future1 = ctrl.wait_for_completion();
    let mut future2 = ctrl.wait_for_completion();
    let _ = future1.poll_unpin(&mut Context::from_waker(&waker));
    let _ = future2.poll_unpin(&mut Context::from_waker(&waker));
    ctrl.trigger_shutdown();
    assert_eq!(counter.get(), 2);
}

// double poll should trigger last waker
#[test]
fn test_07() {
    let ctrl = ShutdownController::new(false);
    let (counter1, waker1) = new_waker();
    let (counter2, waker2) = new_waker();
    let mut future1 = ctrl.wait_for_completion();
    let _ = future1.poll_unpin(&mut Context::from_waker(&waker1));
    let _ = future1.poll_unpin(&mut Context::from_waker(&waker2));
    ctrl.trigger_shutdown();
    assert_eq!(counter1.get(), 0);
    assert_eq!(counter2.get(), 1);
}

// drop should deregister
#[test]
fn test_08() {
    let ctrl = ShutdownController::new(false);
    let (counter1, waker1) = new_waker();
    assert_eq!(Arc::strong_count(&counter1), 2);
    {
        let mut future1 = ctrl.wait_for_completion();
        let _ = future1.poll_unpin(&mut Context::from_waker(&waker1));
        drop(future1);
        drop(waker1);
    }
    ctrl.trigger_shutdown();
    assert_eq!(counter1.get(), 0);
    assert_eq!(Arc::strong_count(&counter1), 1);
}

#[test]
fn test_09() {
    let ctrl = ShutdownController::new(false);
    ctrl.trigger_shutdown();
    let waiter = ctrl.waiter();
    let (counter, waker) = new_waker();
    let mut future = waiter.wait_for_shutdown();
    let result = future.poll_unpin(&mut Context::from_waker(&waker));
    assert_eq!(counter.get(), 0);
    assert_eq!(result, Poll::Ready(()));
}

#[test]
fn test_10() {
    let ctrl = ShutdownController::new(false);
    let waiter = ctrl.waiter();
    let (counter, waker) = new_waker();
    let mut future = waiter.wait_for_shutdown();
    let result = future.poll_unpin(&mut Context::from_waker(&waker));
    assert_eq!(counter.get(), 0);
    assert_eq!(result, Poll::Pending);
    ctrl.trigger_shutdown();
    let result = future.poll_unpin(&mut Context::from_waker(&waker));
    assert_eq!(counter.get(), 1);
    assert_eq!(result, Poll::Ready(()));
    let pending_future = future;

    let (counter, waker) = new_waker();
    let mut future = ctrl.wait_for_completion();
    let result = future.poll_unpin(&mut Context::from_waker(&waker));
    assert_eq!(counter.get(), 0);
    assert_eq!(result, Poll::Pending);

    drop(pending_future);
    let result = future.poll_unpin(&mut Context::from_waker(&waker));
    assert_eq!(counter.get(), 1);
    assert_eq!(result, Poll::Ready(()));
}

#[test]
fn test_11() {
    let mut ctrl = ShutdownController::new(true);
    assert_eq!(ctrl.pending_backtraces().unwrap().count(), 0);

    let waiter = ctrl.waiter();
    let (_, waker) = new_waker();
    let mut future = waiter.wait_for_shutdown();
    assert_eq!(ctrl.pending_backtraces().unwrap().count(), 0);
    let _ = future.poll_unpin(&mut Context::from_waker(&waker));
    assert_eq!(ctrl.pending_backtraces().unwrap().count(), 1);
    drop(future);
    assert_eq!(ctrl.pending_backtraces().unwrap().count(), 0);
}

#[test]
fn test_12() {
    let mut ctrl = ShutdownController::new(true);
    assert_eq!(ctrl.pending_backtraces().unwrap().count(), 0);
    let (_, waker) = new_waker();

    let waiter = ctrl.waiter();
    let mut busy_waiter = ctrl.waiter();
    busy_waiter.mark_busy();

    let mut futures = Vec::with_capacity(1000);
    for _ in 0..1000 {
        let mut future = waiter.wait_for_shutdown();
        let _ = future.poll_unpin(&mut Context::from_waker(&waker));
        futures.push(future);
        let mut future = busy_waiter.wait_for_shutdown();
        let _ = future.poll_unpin(&mut Context::from_waker(&waker));
        futures.push(future);
    }
    while let Some(future) = futures.pop() {
        drop(future);
        assert_eq!(ctrl.pending_backtraces().unwrap().count(), futures.len());
        // to test that it did not consume the backtraces.
        assert_eq!(ctrl.pending_backtraces().unwrap().count(), futures.len());
    }
}

#[test]
fn test_13() {
    let ctrl = ShutdownController::new(false);
    let (counter, waker) = new_waker();

    let (_, com_waker) = new_waker();
    let mut com_future = ctrl.wait_for_completion();
    let result = com_future.poll_unpin(&mut Context::from_waker(&com_waker));
    assert_eq!(result, Poll::Pending);

    let waiter = ctrl.waiter();
    let mut busy_waiter = ctrl.waiter();
    busy_waiter.mark_busy();

    let mut futures = Vec::with_capacity(1000);
    for _ in 0..1000 {
        let mut future = waiter.wait_for_shutdown();
        let _ = future.poll_unpin(&mut Context::from_waker(&waker));
        futures.push(future);
        let mut future = busy_waiter.wait_for_shutdown();
        let _ = future.poll_unpin(&mut Context::from_waker(&waker));
        futures.push(future);

        let result = com_future.poll_unpin(&mut Context::from_waker(&com_waker));
        assert_eq!(result, Poll::Pending);
    }

    ctrl.trigger_shutdown();
    assert_eq!(counter.get(), futures.len());

    while let Some(_future) = futures.pop() {
        let result = com_future.poll_unpin(&mut Context::from_waker(&com_waker));
        assert_eq!(result, Poll::Pending);
    }

    let result = com_future.poll_unpin(&mut Context::from_waker(&com_waker));
    assert_eq!(result, Poll::Ready(()));
}

#[test]
fn test_14() {
    let mut ctrl = ShutdownController::new(true);
    assert_eq!(ctrl.pending_backtraces().unwrap().count(), 0);

    let waiter = ctrl.waiter();
    let (_, waker) = new_waker();
    let mut future = waiter.wait_for_shutdown();
    assert_eq!(ctrl.pending_backtraces().unwrap().count(), 0);
    let _ = future.poll_unpin(&mut Context::from_waker(&waker));
    assert_eq!(ctrl.pending_backtraces().unwrap().count(), 1);
    assert_eq!(ctrl.pending_backtraces().unwrap().count(), 1);
    let (_, waker) = new_waker();
    let _ = future.poll_unpin(&mut Context::from_waker(&waker));
    assert_eq!(ctrl.pending_backtraces().unwrap().count(), 1);
    assert_eq!(ctrl.pending_backtraces().unwrap().count(), 1);
    drop(future);
    assert_eq!(ctrl.pending_backtraces().unwrap().count(), 0);
}

#[test]
fn test_15() {
    let ctrl = ShutdownController::new(false);
    let (counter, waker) = new_waker();

    let waiter = ctrl.waiter();
    let handles = repeat((waiter, waker))
        .take(4)
        .map(|(waiter, waker)| {
            std::thread::spawn(move || {
                let mut futures = Vec::new();

                for _ in 0..100 {
                    let mut future = waiter.wait_for_shutdown();
                    let _ = future.poll_unpin(&mut Context::from_waker(&waker));
                    futures.push(future);
                }

                drop(waker);

                futures::executor::block_on(waiter.wait_for_shutdown());

                while futures.pop().is_some() {
                    std::thread::sleep(Duration::from_millis(1));
                }
            })
        })
        .collect::<Vec<_>>();

    let (_, com_waker) = new_waker();
    let mut com_future = ctrl.wait_for_completion();
    let result = com_future.poll_unpin(&mut Context::from_waker(&com_waker));
    assert_eq!(result, Poll::Pending);

    std::thread::sleep(Duration::from_millis(500)); // wait until everything is inserted.
    let num_wakers = Arc::strong_count(&counter);

    ctrl.trigger_shutdown();

    handles
        .into_iter()
        .for_each(|handle| handle.join().unwrap());

    futures::executor::block_on(ctrl.wait_for_completion());

    let result = com_future.poll_unpin(&mut Context::from_waker(&com_waker));
    assert_eq!(result, Poll::Ready(()));
    assert_eq!(counter.get(), num_wakers - 1);

    // new futures.
    let (_, com_waker) = new_waker();
    let mut com_future = ctrl.wait_for_completion();
    let result = com_future.poll_unpin(&mut Context::from_waker(&com_waker));
    assert_eq!(result, Poll::Ready(()));
}

#[test]
fn test_16() {
    let ctrl = ShutdownController::new(false);
    ctrl.trigger_shutdown();
    let waiter = ctrl.waiter();
    let (counter, waker) = new_waker();
    let mut future = waiter.wait_for_shutdown_owned();
    let result = future.poll_unpin(&mut Context::from_waker(&waker));
    assert_eq!(counter.get(), 0);
    assert_eq!(result, Poll::Ready(()));
}

#[test]
fn test_17() {
    let ctrl = ShutdownController::new(false);
    let waiter = ctrl.waiter();
    let (counter, waker) = new_waker();
    let mut future = waiter.wait_for_shutdown_owned();
    let result = future.poll_unpin(&mut Context::from_waker(&waker));
    assert_eq!(counter.get(), 0);
    assert_eq!(result, Poll::Pending);
    ctrl.trigger_shutdown();
    let result = future.poll_unpin(&mut Context::from_waker(&waker));
    assert_eq!(counter.get(), 1);
    assert_eq!(result, Poll::Ready(()));
    let pending_future = future;

    let (counter, waker) = new_waker();
    let mut future = ctrl.wait_for_completion();
    let result = future.poll_unpin(&mut Context::from_waker(&waker));
    assert_eq!(counter.get(), 0);
    assert_eq!(result, Poll::Pending);

    drop(pending_future);
    let result = future.poll_unpin(&mut Context::from_waker(&waker));
    assert_eq!(counter.get(), 1);
    assert_eq!(result, Poll::Ready(()));
}
