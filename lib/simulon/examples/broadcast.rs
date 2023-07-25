use std::{cell::RefCell, rc::Rc, time::Duration};

use fxhash::{FxHashMap, FxHashSet};
use rand::Rng;
use rand_chacha::ChaCha8Rng;
use rand_core::SeedableRng;
use serde::{Deserialize, Serialize};
use simulon::{api, simulation::SimulationBuilder};

#[derive(Serialize, Deserialize, Debug)]
enum Message {
    Advr(usize),
    Want(usize),
    Payload(usize, Vec<u8>),
}

#[derive(Default)]
struct NodeState {
    conns: FxHashMap<api::RemoteAddr, BroadcastConnection>,
    messages: FxHashMap<usize, Vec<u8>>,
}

struct BroadcastConnection {
    writer: api::OwnedWriter,
    seen: FxHashSet<usize>,
}

impl NodeState {
    fn handle_message_internal(&mut self, id: usize, payload: Vec<u8>) {
        if self.messages.insert(id, payload.clone()).is_none() {
            api::emit(String::from_utf8(payload).unwrap());
        }

        for (_addr, conn) in self.conns.iter_mut() {
            if conn.seen.contains(&id) {
                continue;
            }

            conn.writer.write(&Message::Advr(id));
        }
    }

    pub fn handle_message_from_client(&mut self, id: usize, payload: Vec<u8>) {
        self.handle_message_internal(id, payload);
    }

    pub fn handle_message(&mut self, sender: api::RemoteAddr, id: usize, payload: Vec<u8>) {
        let conn = self.conns.get_mut(&sender).unwrap();
        conn.seen.insert(id);

        self.handle_message_internal(id, payload);
    }

    pub fn handle_advr(&mut self, sender: api::RemoteAddr, id: usize) {
        let conn = self.conns.get_mut(&sender).unwrap();

        conn.seen.insert(id);

        // If we already have the message move on.
        if self.messages.contains_key(&id) {
            return;
        }

        conn.writer.write(&Message::Want(id));
    }

    pub fn handle_want(&mut self, sender: api::RemoteAddr, id: usize) {
        if let Some(payload) = self.messages.get(&id) {
            let conn = self.conns.get_mut(&sender).unwrap();
            conn.seen.insert(id);
            conn.writer.write(&Message::Payload(id, payload.clone()));
        }
    }
}

impl From<api::OwnedWriter> for BroadcastConnection {
    fn from(writer: api::OwnedWriter) -> Self {
        Self {
            writer,
            seen: FxHashSet::default(),
        }
    }
}

type NodeStateRef = Rc<RefCell<NodeState>>;

/// Start a node.
async fn run_node(n: usize) {
    let state = Rc::new(RefCell::new(NodeState::default()));

    // The event loop for accepting connections from the peers.
    api::spawn(listen_for_connections(n, state.clone()));

    // Make the connections.
    api::spawn(make_connections(n, state));
}

async fn listen_for_connections(n: usize, state: NodeStateRef) {
    let mut listener = api::listen(80);
    while let Some(conn) = listener.accept().await {
        if n == *conn.remote() {
            api::spawn(handle_client_connection(state.clone(), conn));
        } else {
            api::spawn(handle_connection(state.clone(), conn));
        }
    }
}

async fn make_connections(n: usize, state: NodeStateRef) {
    let index = *api::RemoteAddr::whoami();
    let connect_to = (index + 1) % n;
    let addr = api::RemoteAddr::from_global_index(connect_to);
    let conn = api::connect(addr, 80).await.expect("Could not connect.");
    handle_connection(state, conn).await;
}

/// Handle the connection that is made from a client.
async fn handle_client_connection(state: NodeStateRef, conn: api::Connection) {
    let (mut reader, _writer) = conn.split();
    let msg = reader.recv::<Message>().await.unwrap();
    if let Message::Payload(id, payload) = msg {
        state.borrow_mut().handle_message_from_client(id, payload);
    } else {
        panic!("unexpected.");
    }
}

/// Handle the connection that is made from another node.
async fn handle_connection(state: NodeStateRef, conn: api::Connection) {
    let remote = conn.remote();
    let (mut reader, writer) = conn.split();

    // Insert the writer half of this connection into the state.
    let b_conn: BroadcastConnection = writer.into();
    state.borrow_mut().conns.insert(remote, b_conn);

    while let Some(msg) = reader.recv::<Message>().await {
        match msg {
            Message::Payload(id, payload) => {
                state.borrow_mut().handle_message(remote, id, payload);
            },
            Message::Advr(id) => {
                state.borrow_mut().handle_advr(remote, id);
            },
            Message::Want(id) => {
                state.borrow_mut().handle_want(remote, id);
            },
        }
    }
}

/// Start a client loop which picks a random node and sends a message to it every
/// few seconds.
async fn run_client(n: usize) {
    let mut rng = ChaCha8Rng::from_seed([0; 32]);

    for i in 0.. {
        let index = rng.gen_range(0..n);
        let addr = api::RemoteAddr::from_global_index(index);

        let mut conn = api::connect(addr, 80).await.expect("Connection failed.");

        let msg = format!("message {i}");
        conn.write(&Message::Payload(i, msg.into()));

        api::sleep(Duration::from_secs(5)).await;
    }
}

fn exec(n: usize) {
    if n == *api::RemoteAddr::whoami() {
        api::spawn(run_client(n));
    } else {
        api::spawn(run_node(n));
    }
}

pub fn main() {
    const N: usize = 1500;

    let report = SimulationBuilder::new(|| exec(N))
        .with_nodes(N + 1)
        // .set_latency_provider(simulon::latency::ConstLatencyProvider(
        //     Duration::from_millis(1),
        // ))
        .set_node_metrics_rate(Duration::ZERO)
        .enable_progress_bar()
        .run(Duration::from_secs(120));

    println!("{:#?}", report.log);
}
