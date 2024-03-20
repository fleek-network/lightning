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
use lightning_interfaces::schema::broadcast::{Advr, Frame, Message, MessageInternedId, Want};
use lightning_interfaces::schema::{AutoImplSerde, LightningMessage};
use lightning_interfaces::types::{NodeIndex, Topic};
use serde::{Deserialize, Serialize};
use tokio::sync::oneshot;

const VALID_SIGN: NodeSignature = NodeSignature([0; 64]);

thread_local! {
    static RUNTIME: Runtime = Runtime::new();
}

struct Runtime {
    clock: RefCell<ClockInfo>,
    executor: RefCell<LocalPool>,
    wakers: RefCell<BTreeMap<u128, Vec<oneshot::Sender<()>>>>,
}

struct ClockInfo {
    started: Instant,
    time: u128,
}

impl Runtime {
    pub fn new() -> Self {
        Self {
            clock: RefCell::new(ClockInfo {
                started: Instant::now(),
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
        let out = Instant::now() - clock_info.started;
        clock_info.time + out.as_nanos()
    }

    /// Run the runtime until there is no more async progress to be made. However this does not
    /// go over sleeps. So probably in the broadcast event loop codebase this will return at a
    /// sleep.
    pub fn run_until_stalled(&self) {
        let started = Instant::now();
        self.clock.borrow_mut().started = started;
        self.executor.borrow_mut().run_until_stalled();
        self.clock.borrow_mut().time = (Instant::now() - started).as_nanos();
    }

    /// Drive the event loop forward and wake up the first sleep futures at the earliest time, but
    /// do not make further progress. To actually perform the code after the sleep.await, you should
    /// call this function or `run_until_stalled` again.
    pub fn fast_forward(&self) -> Duration {
        let started = self.now();
        self.run_until_stalled();
        if let Some((time, wakers)) = self.wakers.borrow_mut().pop_first() {
            self.clock.borrow_mut().time += time;
            for waker in wakers {
                waker.send(()).expect("Could not wake up sleep");
            }
        }
        Duration::from_nanos((self.now() - started) as u64)
    }

    /// Run the entire runtime to completion waking up all the sleeps and driving them forward
    /// as well.
    pub fn run_to_completion(&self) {
        loop {
            self.fast_forward();
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
    outgoing: RefCell<VecDeque<Out>>,
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
        outgoing.push_front(Out::ToAll(ToAll {
            frame,
            filter: Box::new(filter),
        }));
    }

    fn send_to_one(&self, node: NodeIndex, payload: Bytes) {
        let mut outgoing = self.inner.outgoing.borrow_mut();
        let frame = Frame::decode(&payload).unwrap();
        outgoing.push_front(Out::ToOne(ToOne {
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
        frame.encode(&mut payload).expect("Faield to serialize");
        let mut ingested = self.inner.ingested.borrow_mut();
        ingested.push_front((from, payload.into()));
    }

    pub fn take_messages(&self) -> VecDeque<Out> {
        std::mem::take(&mut *self.inner.outgoing.borrow_mut())
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct ExampleMessage {
    id: usize,
}

impl AutoImplSerde for ExampleMessage {}

impl From<ExampleMessage> for Bytes {
    fn from(value: ExampleMessage) -> Self {
        let bytes = bincode::serialize(&value).unwrap();
        let mut buf = BytesMut::with_capacity(bytes.len());
        buf.put_slice(&bytes);
        buf.into()
    }
}

fn spawn_context() -> (
    oneshot::Sender<()>,
    PubSubI<ExampleMessage>,
    ControlledBackend,
) {
    let backend = ControlledBackend::default();
    let ctx = Context::new(Database::default(), backend.clone());
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let pubsub = PubSubI::<ExampleMessage>::new(Topic::Debug, ctx.get_command_sender());
    RUNTIME.with(|rt| {
        rt.spawn(async move {
            ctx.run(shutdown_rx).await;
        })
    });
    (shutdown_tx, pubsub, backend)
}

#[test]
fn demo() {
    let (_shutdown_tx, pubsub, backend) = spawn_context();

    let msg = Message {
        origin: 1,
        signature: VALID_SIGN,
        topic: Topic::Debug,
        timestamp: 0,
        payload: Bytes::from(ExampleMessage { id: 0 }).into(),
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
