use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, RwLock},
};

use tokio::{
    macros::support::poll_fn,
    net::UdpSocket,
    select,
    sync::{
        mpsc,
        mpsc::{Receiver, Sender},
        oneshot,
        oneshot::{Receiver as OneshotReceiver, Sender as OneshotSender},
        Mutex,
    },
};
use tokio_util::time::DelayQueue;

use crate::{
    query::{NodeInfo, Response, ValueHash},
    table::TableQuery,
};

pub async fn start_server(
    // Send lookup responses.
    response_tx: Sender<LookupResponse>,
    // Receive lookup requests.
    mut request_rx: Receiver<LookupRequest>,
    // Receive responses from the network routed by handler.
    mut network_rx: Receiver<Response>,
    // Send queries to table server.
    table_tx: Sender<TableQuery>,
    // Socket to send queries over the network.
    socket: Arc<UdpSocket>,
) {
    let main_server = LookUpServer::new(socket, response_tx);
    let mut expired: DelayQueue<u64> = Default::default();
    let (server_tx, mut server_rx) = mpsc::channel(10000);
    loop {
        let mut server = main_server.clone();
        select! {
            request = request_rx.recv() => {
                let request = request.unwrap();
                if server.pending.get(&request.target).is_none() {
                    let (task_tx, task_rx) = mpsc::channel(10000);
                    server.pending.insert(
                        request.target,
                        LookupHandle { request: request.clone(), task_tx }
                    );
                    let lookup = LookUp {
                        value: request.target,
                        table_tx: table_tx.clone(),
                        server_tx: server_tx.clone(),
                        server_rx:task_rx,
                        inflight: HashMap::new(),
                    };
                    tokio::spawn(run_lookup(lookup));
                }
            }
            network_response = network_rx.recv() => {}
            exp = poll_fn(|cx| expired.poll_expired(cx)) => {}
            update = server_rx.recv() => {}
        }
    }
}

#[derive(Clone)]
pub struct LookupHandle {
    request: LookupRequest,
    task_tx: Sender<Response>,
}

#[derive(Clone)]
pub struct LookupRequest {
    pub target: ValueHash,
}

pub struct LookupResponse {
    pub target: ValueHash,
    pub nodes: Vec<NodeInfo>,
}

async fn run_lookup(_: LookUp) {
    todo!()
}

pub struct LookUp {
    value: ValueHash,
    // Send queries to table server.
    table_tx: Sender<TableQuery>,
    // Channel to send messages to server.
    // At the moment, we just tell the server we're done.
    server_tx: Sender<ValueHash>,
    // Receive messages from server.
    server_rx: Receiver<Response>,
    // Inflight requests.
    inflight: HashMap<u64, LookUp>,
}

#[derive(Clone)]
pub struct LookUpServer {
    // Ongoing lookups.
    pending: HashMap<ValueHash, LookupHandle>,
    // Socket to send queries over the network.
    socket: Arc<UdpSocket>,
    // Send lookup responses.
    response_tx: Sender<LookupResponse>,
}

impl LookUpServer {
    fn new(socket: Arc<UdpSocket>, response_tx: Sender<LookupResponse>) -> Self {
        Self {
            socket,
            response_tx,
            pending: HashMap::new(),
        }
    }
}
