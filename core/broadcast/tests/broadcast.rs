#![allow(unused, dead_code)] // file is wip
use std::cell::RefCell;
use std::collections::{BTreeMap, VecDeque};
use std::rc::Rc;
use std::time::{Duration, Instant};

use bytes::{BufMut, Bytes, BytesMut};
use fleek_crypto::NodeSignature;
use futures::executor::LocalPool;
use futures::task::LocalSpawnExt;
use futures::Future;
use lightning_broadcast::{BroadcastBackend, Context, Database, PubSubI};
use lightning_interfaces::schema::broadcast::{Advr, Frame, Message, MessageInternedId, Want};
use lightning_interfaces::schema::{AutoImplSerde, LightningMessage};
use lightning_interfaces::types::{NodeIndex, Topic};
use serde::{Deserialize, Serialize};
use tokio::sync::oneshot;

const VALID_SIGN: NodeSignature = NodeSignature([0; 64]);

thread_local! {
    static RUNTIME: RefCell<Runtime> = RefCell::new(Runtime::new());
}

struct Runtime {
    started: Instant,
    time: u128,
    executor: LocalPool,
    wakers: BTreeMap<u128, Vec<oneshot::Sender<()>>>,
}

impl Runtime {
    pub fn new() -> Self {
        Self {
            started: Instant::now(),
            time: 0,
            executor: LocalPool::new(),
            wakers: BTreeMap::new(),
        }
    }

    pub fn spawn<Fut>(&mut self, future: Fut)
    where
        Fut: Future<Output = ()> + 'static,
    {
        self.executor
            .spawner()
            .spawn_local(future)
            .expect("Failed to spawn.");
    }

    // Returns the time in nanoseconds.
    pub fn now(&mut self) -> u128 {
        let out = Instant::now() - self.started;
        self.time + out.as_nanos()
    }

    /// Run the runtime until there is no more async progress to be made. However this does not
    /// go over sleeps. So probably in the broadcast event loop codebase this will return at a
    /// sleep.
    pub fn run_until_stalled(&mut self) {
        self.started = Instant::now();
        self.executor.run_until_stalled();
        self.time = (Instant::now() - self.started).as_nanos();
    }

    /// Drive the event loop forward and wake up the first sleep futures at the earliest time, but
    /// do not make further progress. To actually perform the code after the sleep.await, you should
    /// call this function or `run_until_stalled` again.
    pub fn fast_forward(&mut self) -> Duration {
        let started = self.now();
        self.run_until_stalled();
        if let Some((time, wakers)) = self.wakers.pop_first() {
            self.time += time;
            for waker in wakers {
                waker.send(()).expect("Could not wake up sleep");
            }
        }
        Duration::from_nanos((self.now() - started) as u64)
    }

    /// Run the entire runtime to completion waking up all the sleeps and driving them forward
    /// as well.
    pub fn run_to_completion(&mut self) {
        loop {
            self.fast_forward();
            if self.wakers.is_empty() {
                break;
            }
        }
    }

    pub fn sleep(&mut self, duration: Duration) -> oneshot::Receiver<()> {
        let (tx, rx) = oneshot::channel();
        let wakup_time = self.time + duration.as_nanos();
        self.wakers.entry(wakup_time).or_default().push(tx);
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

enum Out {
    ToAll {
        frame: Frame,
        filter: Box<dyn Fn(NodeIndex) -> bool + Send + Sync + 'static>,
    },
    ToOne {
        frame: Frame,
        destination: NodeIndex,
    },
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
        outgoing.push_front(Out::ToAll {
            frame,
            filter: Box::new(filter),
        });
    }

    fn send_to_one(&self, node: NodeIndex, payload: Bytes) {
        let mut outgoing = self.inner.outgoing.borrow_mut();
        let frame = Frame::decode(&payload).unwrap();
        outgoing.push_front(Out::ToOne {
            frame,
            destination: node,
        });
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
        (RUNTIME.with(|cell| cell.borrow_mut().now()) / 1_000_000) as u64
    }

    async fn sleep(duration: Duration) {
        let rx = RUNTIME.with(|cell| cell.borrow_mut().sleep(duration));
        rx.await.expect("Failed to sleep");
    }
}

impl ControlledBackend {
    pub fn push_frame(&mut self, from: NodeIndex, frame: Frame) {
        let mut payload = Vec::new();
        frame.encode(&mut payload).expect("Faield to serialize");
        let mut ingested = self.inner.ingested.borrow_mut();
        ingested.push_front((from, payload.into()));
    }

    pub fn take_messages(&mut self) -> VecDeque<Out> {
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
    RUNTIME.with(|cell| {
        cell.borrow_mut().spawn(async move {
            ctx.run(shutdown_rx).await;
        })
    });
    (shutdown_tx, pubsub, backend)
}

#[test]
fn demo() {
    let (_shutdown_tx, pubsub, backend) = spawn_context();
}
