use lightning_interfaces::{ShutdownController, Subscriber};
use tokio::sync::broadcast;
use tokio::test;
use tokio::time::{sleep, timeout, Duration};

use crate::BroadcastSub;

#[test]
async fn sub_is_cancel_safe() {
    let ctrl = ShutdownController::new(false);
    let (tx, _) = broadcast::channel::<u8>(3);
    let mut sub1 = BroadcastSub(tx.subscribe(), ctrl.waiter().into());
    let mut sub2 = BroadcastSub(tx.subscribe(), ctrl.waiter().into());
    tx.send(1).expect("Failed to send");

    let mut sub1_got = false;
    let mut sub2_got = false;

    for _ in 0..2 {
        tokio::select! {
            _ = sub1.recv() => {
                sub1_got = true;
            },
            _ = sub2.recv() => {
                sub2_got = true;
            },
            _ = sleep(Duration::from_millis(10)) => {
                continue;
            }
        }
    }

    assert!(sub1_got, "sub1 to get the notification.");
    assert!(sub2_got, "sub2 to get the notification.");
}

#[test]
async fn sub_skips_lag() {
    let ctrl = ShutdownController::new(false);
    let (tx, _) = broadcast::channel::<u8>(3);
    let mut sub1 = BroadcastSub(tx.subscribe(), ctrl.waiter().into());

    for i in 0..10 {
        tx.send(i).unwrap();
    }

    let ret = timeout(Duration::from_millis(10), sub1.recv()).await;
    assert!(matches!(ret, Ok(Some(x)) if x > 3));
}

#[test]
async fn sub_recv_last_works() {
    let ctrl = ShutdownController::new(false);
    let (tx, _) = broadcast::channel::<u8>(3);
    let mut sub1 = BroadcastSub(tx.subscribe(), ctrl.waiter().into());

    tx.send(0).unwrap();
    tx.send(1).unwrap();
    tx.send(2).unwrap();

    let ret = timeout(Duration::from_millis(10), sub1.last()).await;
    assert_eq!(ret, Ok(Some(2)));

    let ret = timeout(Duration::from_millis(10), sub1.last()).await;
    assert!(ret.is_err(), "To time out");

    // Test lagged.
    for i in 0..10 {
        tx.send(i).unwrap();
    }

    let ret = timeout(Duration::from_millis(10), sub1.last()).await;
    assert_eq!(ret, Ok(Some(9)));

    tokio::spawn(async move {
        sleep(Duration::from_micros(10)).await;
        tx.send(10).unwrap();
    });

    let ret = timeout(Duration::from_millis(100), sub1.last()).await;
    assert_eq!(ret, Ok(Some(10)));
}

#[test]
async fn sub_recv_last_works_sync_drop() {
    let ctrl = ShutdownController::new(false);
    let (tx, _) = broadcast::channel::<u8>(3);
    let mut sub1 = BroadcastSub(tx.subscribe(), ctrl.waiter().into());
    drop(tx);

    let ret = timeout(Duration::from_millis(10), sub1.last()).await;
    assert_eq!(ret, Ok(None));
}

#[test]
async fn sub_recv_last_works_async_drop() {
    let ctrl = ShutdownController::new(false);
    let (tx, _) = broadcast::channel::<u8>(3);
    let mut sub1 = BroadcastSub(tx.subscribe(), ctrl.waiter().into());

    tokio::spawn(async move {
        sleep(Duration::from_micros(10)).await;
        drop(tx);
    });

    let ret = timeout(Duration::from_millis(100), sub1.last()).await;
    assert_eq!(ret, Ok(None));
}
