use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

use bytes::{BufMut, Bytes, BytesMut};
use fleek_crypto::{NodePublicKey, NodeSecretKey, SecretKey};
use lightning_broadcast::{Context, Database, PubSubI, SimulonBackend};
use lightning_interfaces::schema::AutoImplSerde;
use lightning_interfaces::types::Topic;
use lightning_interfaces::PubSub;
use lightning_topology::{
    build_latency_matrix,
    suggest_connections_from_latency_matrix,
    Connections,
};
use rand::Rng;
use serde::{Deserialize, Serialize};
use simulon::api::{OwnedWriter, RemoteAddr};
use simulon::latency::LatencyProvider;
use simulon::simulation::SimulationBuilder;

type NodeIndex = u32;
type TopologyConnections = Arc<Connections>;
type SecretKeyMappings = Arc<HashMap<usize, NodeSecretKey>>;
type KeyMappings = Arc<HashMap<NodeIndex, NodePublicKey>>;
type IndexMappings = Arc<HashMap<NodePublicKey, NodeIndex>>;

const N: usize = 1500;

async fn exec(n: usize) {
    let node_index = *RemoteAddr::whoami();
    if node_index == n {
        return run_client(n).await;
    }

    let conns = simulon::api::with_state(TopologyConnections::clone);
    let conns = conns.get(node_index);

    let peers = conns
        .iter()
        .flatten()
        .copied()
        .filter(|index| *index != node_index)
        .collect::<HashSet<_>>();

    let secret_keys = simulon::api::with_state(SecretKeyMappings::clone);
    let index_to_key = simulon::api::with_state(KeyMappings::clone);
    let key_to_index = simulon::api::with_state(IndexMappings::clone);

    let secret_key = secret_keys.get(&node_index).unwrap();

    let (conn_tx, mut conn_rx) = tokio::sync::mpsc::channel(16);

    // listener task for node connections.
    let tmp = conn_tx.clone();
    simulon::api::spawn(async move {
        let conn_tx = tmp;
        let mut listener = simulon::api::listen(80);
        while let Some(conn) = listener.accept().await {
            conn_tx.send(conn).await.expect("Failed to send");
        }
    });

    // dial tasks
    for peer in &peers {
        if *peer > node_index {
            continue;
        }
        let conn_tx = conn_tx.clone();
        let peer_index = *peer;
        simulon::api::spawn(async move {
            let addr = simulon::api::RemoteAddr::from_global_index(peer_index);
            let conn = simulon::api::connect(addr, 80)
                .await
                .expect("Could not connect.");
            conn_tx.send(conn).await.expect("Failed to send");
        });
    }

    // message task
    let (msg_sender_tx, mut msg_sender_rx) = tokio::sync::mpsc::channel(1024);
    let (msg_recv_tx, msg_recv_rx) = tokio::sync::mpsc::channel(1024);

    let backend = SimulonBackend::new(
        msg_sender_tx,
        msg_recv_rx,
        peers,
        key_to_index,
        index_to_key,
    );
    let ctx = Context::new(Database::default(), secret_key.clone(), backend);
    let command_sender = ctx.get_command_sender();
    let mut pub_sub = PubSubI::<Message>::new(Topic::Debug, command_sender);

    let (cmd_sender_tx, mut cmd_sender_rx) = tokio::sync::mpsc::channel::<(usize, Message)>(1024);
    simulon::api::spawn(async move {
        loop {
            tokio::select! {
                msg = pub_sub.recv() => {
                    if let Some(msg) = msg {
                        simulon::api::emit(msg.id.to_string());
                    }
                }
                msg = cmd_sender_rx.recv() => {
                    if let Some((peer, msg)) = msg {
                        if peer == N {
                            // this message is from the client
                            simulon::api::emit(msg.id.to_string());
                            pub_sub
                                .send(&msg, None)
                                .await
                                .expect("Failed to broadcast message");
                        }
                    } else {
                        // should we do anything here?
                    }
                }
            }
        }
    });

    simulon::api::spawn(async move {
        let mut writers = HashMap::<usize, OwnedWriter>::new();
        loop {
            tokio::select! {
                conn = conn_rx.recv() => {
                    if let Some(conn) = conn {
                        let peer_index = *conn.remote();
                        let (mut reader, writer) = conn.split();
                        let msg_recv_tx_ = msg_recv_tx.clone();
                        let cmd_sender_tx_ = cmd_sender_tx.clone();
                        simulon::api::spawn(async move {
                            let index = *reader.remote();
                            while let Some(msg) = reader.recv::<Message>().await {
                                cmd_sender_tx_.send((index, msg.clone())).await.expect("Failed to send");
                                msg_recv_tx_.send((index as NodeIndex, msg.into())).await.expect("Failed to send");
                            }
                        });
                        writers.insert(peer_index, writer);
                    }
                }
                msg = msg_sender_rx.recv() => {
                    if let Some((peer, payload)) = msg {
                        if let Some(writer) = writers.get_mut(&peer) {
                            writer.write::<Bytes>(&payload);
                        }
                    }

                }
            }
        }
    });

    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
    simulon::api::spawn(async move {
        ctx.run(shutdown_rx).await;
        let _tx = shutdown_tx; // prevent the node from shutting down by keeping the shutdown sender alive
    });
}

/// Start a client loop which picks a random node and sends a message to it every
/// few seconds.
async fn run_client(n: usize) {
    let mut rng = rand::thread_rng();
    simulon::api::sleep(Duration::from_secs(5)).await;

    for i in 0.. {
        let index = rng.gen_range(0..n);
        let addr = simulon::api::RemoteAddr::from_global_index(index);

        let mut conn = simulon::api::connect(addr, 80)
            .await
            .expect("Connection failed.");

        let msg = Message { id: i };
        conn.write(&msg);

        simulon::api::sleep(Duration::from_secs(5)).await;
    }
}

pub fn main() {
    let secret_keys: HashMap<usize, NodeSecretKey> = (0..N)
        .map(|index| (index, NodeSecretKey::generate()))
        .collect();
    let index_to_key: HashMap<NodeIndex, NodePublicKey> = secret_keys
        .iter()
        .map(|(index, key)| (*index as NodeIndex, key.to_pk()))
        .collect();
    let key_to_index: HashMap<NodePublicKey, NodeIndex> = index_to_key
        .iter()
        .map(|(index, key)| (*key, *index))
        .collect();

    let mut lat_provider = simulon::latency::ConstLatencyProvider(Duration::from_millis(1));

    let mut latencies = HashMap::new();
    for i in 0..(N - 1) {
        for j in (i + 1)..N {
            let lat = lat_provider.get(i, j);
            let key_i = index_to_key.get(&(i as NodeIndex)).unwrap();
            let key_j = index_to_key.get(&(j as NodeIndex)).unwrap();
            latencies.insert((*key_i, *key_j), lat);
        }
    }

    let valid_pubkeys: BTreeSet<NodePublicKey> = index_to_key.values().copied().collect();
    let dummy_key = NodeSecretKey::generate().to_pk();
    let (matrix, mappings, _) = build_latency_matrix(dummy_key, latencies, valid_pubkeys);
    let connections = suggest_connections_from_latency_matrix(0, matrix, &mappings, 9, 8);

    let time = std::time::Instant::now();
    SimulationBuilder::new(|| simulon::api::spawn(exec(N)))
        .with_nodes(N + 1)
        .set_latency_provider(lat_provider)
        .with_state(Arc::new(connections))
        .with_state(Arc::new(secret_keys))
        .with_state(Arc::new(index_to_key))
        .with_state(Arc::new(key_to_index))
        .set_node_metrics_rate(Duration::ZERO)
        .enable_progress_bar()
        .run(Duration::from_secs(120));
    println!("Took {} ms", time.elapsed().as_millis());
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Message {
    id: usize,
}

impl AutoImplSerde for Message {}

impl From<Message> for Bytes {
    fn from(value: Message) -> Self {
        let bytes = bincode::serialize(&value).unwrap();
        let mut buf = BytesMut::with_capacity(bytes.len());
        buf.put_slice(&bytes);
        buf.into()
    }
}
