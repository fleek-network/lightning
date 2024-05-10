use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use lightning_broadcast::{Context, Database, PubSubI, SimulonBackend};
use lightning_interfaces::schema::AutoImplSerde;
use lightning_interfaces::types::Topic;
use lightning_interfaces::{PubSub, ShutdownController};
use lightning_topology::Connections;
use rand::Rng;
use serde::{Deserialize, Serialize};
use simulon::api::{OwnedWriter, RemoteAddr};

pub type NodeIndex = u32;
pub type TopologyConnections = Arc<Connections>;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Message {
    id: usize,
    payload: Vec<u8>,
}

impl AutoImplSerde for Message {}

pub async fn exec(n: usize) {
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

    let (conn_tx, mut conn_rx) = tokio::sync::mpsc::channel(16);

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

    assert!(!peers.is_empty());
    let backend = SimulonBackend::new(msg_sender_tx, msg_recv_rx, peers);

    let ctx = Context::new(Database::default(), backend);
    let ctx_command_sender = ctx.get_command_sender();

    // listener task for node + client connections.
    let tmp = conn_tx.clone();
    simulon::api::spawn(async move {
        let conn_tx = tmp;
        let mut listener = simulon::api::listen(80);
        while let Some(mut conn) = listener.accept().await {
            if *conn.remote() == n {
                // Handle the client connection differently.
                let pub_sub = PubSubI::<Message>::new(Topic::Debug, ctx_command_sender.clone());
                simulon::api::spawn(async move {
                    while let Some(msg) = conn.recv::<Message>().await {
                        simulon::api::emit(msg.id.to_string());
                        pub_sub
                            .send(&msg, None)
                            .await
                            .expect("Could not send the message");
                    }
                });
            } else {
                conn_tx.send(conn).await.expect("Failed to send");
            }
        }
    });

    // The task to recv message from the broadcast. This will make broadcast propagate the message
    // further.
    let mut pub_sub = PubSubI::<Message>::new(Topic::Debug, ctx.get_command_sender());
    simulon::api::spawn(async move {
        loop {
            if let Some(msg) = pub_sub.recv().await {
                simulon::api::emit(msg.id.to_string());
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
                        simulon::api::spawn(async move {
                            let index = *reader.remote() as NodeIndex;
                            while let Some(msg) = reader.recv::<Bytes>().await {
                                msg_recv_tx_.send((index, msg)).await.expect("Failed to send");
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

    let ctrl = ShutdownController::new(false);
    let waiter = ctrl.waiter();
    simulon::api::spawn(async move {
        ctx.run(waiter).await;
    });
    simulon::api::spawn(async move {
        simulon::api::sleep(Duration::from_secs(1800)).await;
        ctrl.trigger_shutdown();
    })
}

/// Start a client loop which picks a random node and sends a message to it every
/// few seconds.
pub async fn run_client(n: usize) {
    let mut rng = rand::thread_rng();
    simulon::api::sleep(Duration::from_secs(5)).await;

    for i in 0.. {
        let index = rng.gen_range(0..n);
        let addr = simulon::api::RemoteAddr::from_global_index(index);

        let mut conn = simulon::api::connect(addr, 80)
            .await
            .expect("Connection failed.");

        let msg = Message {
            id: i,
            payload: vec![0; 512],
        };
        conn.write(&msg);

        simulon::api::sleep(Duration::from_secs(1)).await;
    }
}
