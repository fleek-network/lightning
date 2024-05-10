#![allow(unused, dead_code)] // file is wip
use std::cell::RefCell;
use std::collections::{BTreeMap, VecDeque};
use std::fmt::Debug;
use std::rc::Rc;
use std::time::{Duration, Instant};

use bytes::{BufMut, Bytes, BytesMut};
use fleek_crypto::NodeSignature;
use futures::executor::{block_on, LocalPool};
use futures::task::LocalSpawnExt;
use futures::Future;
use ink_quill::ToDigest;
use lightning_broadcast::{BroadcastBackend, Context, Database, PubSubI};
use lightning_interfaces::prelude::*;
use lightning_interfaces::schema::broadcast::{Advr, Frame, Message, MessageInternedId, Want};
use lightning_interfaces::schema::{AutoImplSerde, LightningMessage};
use lightning_interfaces::types::{NodeIndex, Topic};
use lightning_interfaces::ShutdownController;
use lightning_test_utils::logging;
use serde::{Deserialize, Serialize};
use tokio::sync::oneshot;

const VALID_SIGN: NodeSignature = NodeSignature([0; 64]);
const ONE_HOUR: Duration = Duration::from_secs(3600);

thread_local! {
    static RUNTIME: Runtime = Runtime::new();
}

struct Runtime {
    clock: RefCell<ClockInfo>,
    executor: RefCell<LocalPool>,
    wakers: RefCell<BTreeMap<u128, Vec<oneshot::Sender<()>>>>,
}

struct ClockInfo {
    // the time the current active execution started. is only Some during excution and now()
    // will be adjusted based on this and self.time.
    started: Option<Instant>,
    time: u128,
}

impl Runtime {
    pub fn new() -> Self {
        Self {
            clock: RefCell::new(ClockInfo {
                started: None,
                time: 0,
            }),
            executor: RefCell::new(LocalPool::new()),
            wakers: RefCell::new(BTreeMap::new()),
        }
    }

    pub fn spawn<Fut>(&self, future: Fut)
    where
        Fut: Future<Output = ()> + 'static,
    {
        self.executor
            .borrow()
            .spawner()
            .spawn_local(future)
            .expect("Failed to spawn.");
    }

    // Returns the time in nanoseconds.
    pub fn now(&self) -> u128 {
        let clock_info = self.clock.borrow_mut();
        if let Some(started) = clock_info.started {
            clock_info.time + (Instant::now() - started).as_nanos()
        } else {
            clock_info.time
        }
    }

    /// Run the runtime until there is no more async progress to be made. However this does not
    /// go over sleeps. So probably in the broadcast event loop codebase this will return at a
    /// sleep.
    pub fn run_until_stalled(&self) {
        // wake up any sleep task that have reached their time.
        let now = self.now();
        let mut wakers_map = self.wakers.borrow_mut();
        while matches!(wakers_map.first_entry(), Some(e) if e.key() <= &now) {
            for waker in wakers_map.pop_first().unwrap().1 {
                let _ = waker.send(());
            }
        }
        drop(wakers_map);

        let started = Instant::now();
        self.clock.borrow_mut().started = Some(started);
        self.executor.borrow_mut().run_until_stalled();
        let mut clock = self.clock.borrow_mut();
        clock.time = (Instant::now() - started).as_nanos();
        clock.started = None;
    }

    /// Drive the event loop forward and wake up the first sleep futures at the earliest time, but
    /// do not make further progress. To actually perform the code after the sleep.await, you should
    /// call this function or `run_until_stalled` again.
    pub fn fast_forward(&self) -> Duration {
        let started = self.now();
        self.run_until_stalled();
        let mut fast_forward = true;
        while fast_forward {
            if let Some((time, wakers)) = self.wakers.borrow_mut().pop_first() {
                self.clock.borrow_mut().time = time;
                for waker in wakers {
                    if waker.send(()).is_ok() {
                        // we woke up at least one sleep sucessfully, so we gotta stop here.
                        fast_forward = false;
                    }
                }
            } else {
                fast_forward = false;
            }
        }
        Duration::from_nanos((self.now() - started) as u64)
    }

    /// Run the entire runtime to completion waking up all the sleeps and driving them forward
    /// as well.
    pub fn run_to_completion(&self) {
        loop {
            self.fast_forward();
            self.run_until_stalled();
            if self.wakers.borrow().is_empty() {
                break;
            }
        }
    }

    pub fn sleep(&self, duration: Duration) -> oneshot::Receiver<()> {
        let (tx, rx) = oneshot::channel();
        let wakup_time = self.now() + duration.as_nanos();
        self.wakers
            .borrow_mut()
            .entry(wakup_time)
            .or_default()
            .push(tx);
        rx
    }
}

#[derive(Clone, Default)]
struct ControlledBackend {
    inner: Rc<ControlledBackendInner>,
}

#[derive(Default)]
struct ControlledBackendInner {
    ingested: RefCell<VecDeque<(NodeIndex, Bytes)>>,
    outgoing: RefCell<Vec<Out>>,
}

struct ToAll {
    pub frame: Frame,
    pub filter: Box<dyn Fn(NodeIndex) -> bool + Send + Sync + 'static>,
}

#[derive(Debug, Clone)]
struct ToOne {
    pub frame: Frame,
    pub destination: NodeIndex,
}

enum Out {
    ToAll(ToAll),
    ToOne(ToOne),
}

impl Debug for Out {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Out::ToAll(ToAll { frame, .. }) => {
                f.debug_struct("ToAll").field("frame", &frame).finish()
            },
            Out::ToOne(ToOne { frame, destination }) => f
                .debug_struct("ToOne")
                .field("frame", &frame)
                .field("destination", destination)
                .finish(),
        }
    }
}

impl BroadcastBackend for ControlledBackend {
    type Pk = ();

    fn send_to_all<F: Fn(NodeIndex) -> bool + Send + Sync + 'static>(
        &self,
        payload: Bytes,
        filter: F,
    ) {
        let mut outgoing = self.inner.outgoing.borrow_mut();
        let frame = Frame::decode(&payload).unwrap();
        outgoing.push(Out::ToAll(ToAll {
            frame,
            filter: Box::new(filter),
        }));
    }

    fn send_to_one(&self, node: NodeIndex, payload: Bytes) {
        let mut outgoing = self.inner.outgoing.borrow_mut();
        let frame = Frame::decode(&payload).unwrap();
        outgoing.push(Out::ToOne(ToOne {
            frame,
            destination: node,
        }));
    }

    fn receive(&mut self) -> impl Future<Output = Option<(NodeIndex, Bytes)>> + Send {
        let msg = self.inner.ingested.borrow_mut().pop_front();
        async move { msg }
    }

    fn get_node_pk(&self, _index: NodeIndex) -> Option<Self::Pk> {
        Some(())
    }

    fn get_our_index(&self) -> Option<NodeIndex> {
        Some(0)
    }

    fn sign(&self, _digest: [u8; 32]) -> NodeSignature {
        VALID_SIGN
    }

    fn verify(_pk: &Self::Pk, signature: &NodeSignature, _digest: &[u8; 32]) -> bool {
        signature.0 == VALID_SIGN.0
    }

    fn report_sat(
        &self,
        _peer: lightning_interfaces::types::NodeIndex,
        _weight: lightning_interfaces::Weight,
    ) {
    }

    fn now() -> u64 {
        (RUNTIME.with(|cell| cell.now()) / 1_000_000) as u64
    }

    async fn sleep(duration: Duration) {
        let rx = RUNTIME.with(|cell| cell.sleep(duration));
        rx.await.expect("Failed to sleep");
    }
}

impl ControlledBackend {
    pub fn push_frame(&self, from: NodeIndex, frame: Frame) {
        let mut payload = Vec::new();
        frame.encode(&mut payload).expect("Failed to serialize");
        let mut ingested = self.inner.ingested.borrow_mut();
        ingested.push_back((from, payload.into()));
    }

    pub fn take_messages(&self) -> Vec<Out> {
        std::mem::take(&mut *self.inner.outgoing.borrow_mut())
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct ExampleMessage {
    id: usize,
}

impl AutoImplSerde for ExampleMessage {}

impl From<ExampleMessage> for Vec<u8> {
    fn from(value: ExampleMessage) -> Self {
        let mut result = Vec::new();
        value.encode(&mut result).unwrap();
        result
    }
}

fn spawn_context(duration: Duration) -> (PubSubI<ExampleMessage>, ControlledBackend) {
    let backend = ControlledBackend::default();
    let ctx = Context::new(Database::default(), backend.clone());
    let ctrl = ShutdownController::new(false);
    let waiter = ctrl.waiter();
    let pubsub = PubSubI::<ExampleMessage>::new(Topic::Debug, ctx.get_command_sender());
    RUNTIME.with(|rt| {
        rt.spawn(async move {
            ControlledBackend::sleep(duration).await;
            ctrl.trigger_shutdown();
        });
        rt.spawn(async move {
            ctx.run(waiter).await;
        })
    });
    (pubsub, backend)
}

fn timeout<F>(duration: Duration, future: F) -> impl Future<Output = Result<F::Output, ()>>
where
    F: Future,
{
    let sleep = RUNTIME.with(|rt| rt.sleep(duration));
    async move {
        tokio::select! {
            _ = sleep => { Err(()) },
            out = future => { Ok(out) }
        }
    }
}

#[test]
fn demo() {
    logging::setup();

    let (pubsub, backend) = spawn_context(ONE_HOUR);

    let msg = Message {
        origin: 1,
        signature: VALID_SIGN,
        topic: Topic::Debug,
        timestamp: 0,
        payload: Vec::from(ExampleMessage { id: 0 }),
    };

    let digest = msg.to_digest();
    backend.push_frame(
        1,
        Frame::Advr(Advr {
            interned_id: 0,
            digest,
        }),
    );
    backend.push_frame(
        2,
        Frame::Advr(Advr {
            interned_id: 0,
            digest,
        }),
    );

    let x = RUNTIME.with(|rt| rt.fast_forward());
    println!("{x:?}");
    let messages = backend.take_messages();
    println!("{:?}", messages);

    let x = RUNTIME.with(|rt| rt.fast_forward());
    println!("{x:?}");
    let messages = backend.take_messages();
    println!("{:?}", messages);

    let x = RUNTIME.with(|rt| rt.fast_forward());
    println!("{x:?}");
    let messages = backend.take_messages();
    println!("{:?}", messages);

    let x = RUNTIME.with(|rt| rt.fast_forward());
    println!("{x:?}");
    let messages = backend.take_messages();
    println!("{:?}", messages);

    let x = RUNTIME.with(|rt| rt.fast_forward());
    println!("{x:?}");
    let messages = backend.take_messages();
    println!("{:?}", messages);

    let x = RUNTIME.with(|rt| rt.fast_forward());
    println!("{x:?}");
    let messages = backend.take_messages();
    println!("{:?}", messages);
}

#[test]
fn test_reject_message() {
    // In this test we send two messages with the same digest to the event loop of the broadcast.
    // The pubsub receiver (outside of broadcast) rejects the first message.
    // After listening to the pubsub again, the pubsub receiver should still receive the second
    // message that was sent.
    let (mut pubsub, backend) = spawn_context(ONE_HOUR);

    let msg = Message {
        origin: 99,
        signature: VALID_SIGN,
        topic: Topic::Debug,
        timestamp: 0,
        payload: ExampleMessage { id: 0 }.into(), // invalid origin
    };
    let digest = msg.to_digest();
    let adv = Frame::Advr(Advr {
        interned_id: 0,
        digest,
    });

    backend.push_frame(1, adv);
    backend.push_frame(1, Frame::Message(msg));

    let x = RUNTIME.with(|rt| rt.fast_forward());

    let msg = Message {
        origin: 2,
        signature: VALID_SIGN,
        topic: Topic::Debug,
        timestamp: 0,
        payload: ExampleMessage { id: 0 }.into(), // valid origin
    };
    let digest = msg.to_digest();
    let adv = Frame::Advr(Advr {
        interned_id: 0,
        digest,
    });

    backend.push_frame(2, adv);
    backend.push_frame(2, Frame::Message(msg));

    let x = RUNTIME.with(|rt| rt.fast_forward());

    RUNTIME.with(|rt| {
        rt.spawn(async move {
            let Ok(event) = timeout(Duration::from_millis(5000), pubsub.recv_event()).await else {
                panic!("Message did not arrive in time");
            };
            let mut event = event.unwrap();
            let msg = event.take().unwrap();
            assert_eq!(msg.id, 0);
            // Reject the message
            event.mark_invalid_sender();

            // Since we rejected the message, we should receive the other message with the same
            // digest
            let Ok(msg) = timeout(Duration::from_millis(5000), pubsub.recv_event()).await else {
                panic!("Message did not arrive in time");
            };
            assert_eq!(msg.unwrap().take().unwrap().id, 0);
        });

        rt.run_to_completion();
    });
}

#[test]
#[should_panic(expected = "finished")]
fn test_propagate_message() {
    // In this test we send two messages with the same digest to the event loop of the broadcast.
    // The pubsub receiver (outside of broadcast) propagates the first message.
    // After listening to the pubsub again, the pubsub receiver should not receive the second
    // message that was sent.
    let (mut pubsub, backend) = spawn_context(ONE_HOUR);

    let msg = Message {
        origin: 99,
        signature: VALID_SIGN,
        topic: Topic::Debug,
        timestamp: 0,
        payload: ExampleMessage { id: 0 }.into(), // invalid origin
    };
    let digest = msg.to_digest();
    let adv = Frame::Advr(Advr {
        interned_id: 0,
        digest,
    });

    backend.push_frame(1, adv);
    backend.push_frame(1, Frame::Message(msg));

    let x = RUNTIME.with(|rt| rt.fast_forward());

    let msg = Message {
        origin: 2,
        signature: VALID_SIGN,
        topic: Topic::Debug,
        timestamp: 0,
        payload: ExampleMessage { id: 0 }.into(), // valid origin
    };
    let digest = msg.to_digest();
    let adv = Frame::Advr(Advr {
        interned_id: 0,
        digest,
    });

    backend.push_frame(2, adv);
    backend.push_frame(2, Frame::Message(msg));

    let x = RUNTIME.with(|rt| rt.fast_forward());

    RUNTIME.with(|rt| {
        rt.spawn(async move {
            let Ok(event) = timeout(Duration::from_millis(5000), pubsub.recv_event()).await else {
                panic!("Message did not arrive in time");
            };
            let mut event = event.unwrap();
            let msg = event.take().unwrap();
            assert_eq!(msg.id, 0);

            // Propagate the message
            event.propagate();

            // Since we propagated the message, we should not receive another message with
            // the same digest
            assert!(pubsub.recv_event().await.is_none());
            panic!("finished");

            // Note: outside of this testground, the call to `recv_event` would not resolve if
            // there aren't anys messages. Since we fast forward the runtime, the context will
            // shutdown.
        });

        rt.run_to_completion();
    });
}
