use std::time::Duration;

use async_channel::{Receiver, Sender};
use bytes::{BufMut, Bytes, BytesMut};
use criterion::{criterion_group, BenchmarkId, Criterion};
use fleek_crypto::{ClientPublicKey, ClientSignature};
use lightning_blockstore::blockstore::Blockstore;
use lightning_handshake::handshake::Handshake;
use lightning_handshake::schema;
use lightning_handshake::transports::mock::dial_mock;
use lightning_interfaces::prelude::*;
use lightning_node::Node;
use lightning_service_executor::shim::ServiceExecutor;
use lightning_service_executor::test_services::io_stress;
use lightning_signer::Signer;
use lightning_test_utils::json_config::JsonConfigProvider;
use lightning_test_utils::keys::EphemeralKeystore;
use serde_json::json;
use tokio::runtime::Runtime;
use tokio::task::JoinHandle;

static MB: usize = 1024 * 1024;

partial_node_components!(TestBinding {
    KeystoreInterface = EphemeralKeystore<Self>;
    SignerInterface = Signer<Self>;
    BlockstoreInterface = Blockstore<Self>;
    ConfigProviderInterface = JsonConfigProvider;
    HandshakeInterface = Handshake<Self>;
    ServiceExecutorInterface = ServiceExecutor<Self>;
});

pub fn delimit_frame(bytes: Bytes) -> Bytes {
    let mut buf = BytesMut::with_capacity(4 + bytes.len());
    buf.put_u32(bytes.len() as u32);
    buf.put(bytes);
    buf.into()
}

/// Setup the node.
async fn setup_node() -> Node<TestBinding> {
    let _ = tokio::fs::remove_dir_all("./ipc").await;
    let config: JsonConfigProvider = json!({
      "handshake": {
        "http_address": "127.0.0.1:4220",
        "transport": [
          {
            "type": "Mock",
            "port": 69
          },
        ],
      },
      "service-executor": {
        "services": [1001],
        "ipc_path": "./ipc"
      },
    })
    .into();

    let node = Node::<TestBinding>::init(config).unwrap();
    node.start().await;
    node
}

async fn establish_connection() -> (Sender<Bytes>, Receiver<Bytes>) {
    dial_mock(69).await.unwrap()
}

/// Perform the handshake over the connection.
async fn perform_handshake(tx: &Sender<Bytes>, rx: &mut Receiver<Bytes>) {
    tx.send(
        schema::HandshakeRequestFrame::Handshake {
            retry: None,
            service: 1001,
            pk: ClientPublicKey([1; 96]),
            pop: ClientSignature([2; 48]),
        }
        .encode(),
    )
    .await
    .unwrap();

    // get the first message.
    let _ = rx.recv().await;
}

async fn io_stress_write(
    chunk_len: usize,
    chunks: usize,
    tx: &Sender<Bytes>,
    rx: &mut Receiver<Bytes>,
) {
    tx.send(
        schema::RequestFrame::ServicePayload {
            bytes: delimit_frame(io_stress::Message::Request { chunk_len, chunks }.encode()),
        }
        .encode(),
    )
    .await
    .unwrap();

    let mut remaining = chunks * chunk_len;
    while remaining > 0 {
        remaining -= rx.recv().await.unwrap().len() - 1;
    }
}

/// Run `n` clients to make the node busy.
fn run_clients(n: usize) -> Vec<JoinHandle<()>> {
    let mut result = Vec::with_capacity(n);

    for _ in 0..n {
        let handle = tokio::spawn(async {
            loop {
                let Ok((tx, rx)) = dial_mock(69).await else {
                    break;
                };

                if tx
                    .send(
                        schema::HandshakeRequestFrame::Handshake {
                            retry: None,
                            service: 1001,
                            pk: ClientPublicKey([1; 96]),
                            pop: ClientSignature([2; 48]),
                        }
                        .encode(),
                    )
                    .await
                    .is_err()
                {
                    break;
                }

                let _ = rx.recv().await;

                if tx
                    .send(
                        schema::RequestFrame::ServicePayload {
                            bytes: delimit_frame(
                                io_stress::Message::Request {
                                    chunk_len: 8,
                                    chunks: 1024,
                                }
                                .encode(),
                            ),
                        }
                        .encode(),
                    )
                    .await
                    .is_err()
                {
                    break;
                }

                let mut remaining = 1024 * 8;
                while remaining > 0 {
                    if let Ok(bytes) = rx.recv().await {
                        remaining -= bytes.len() - 1;
                    } else {
                        return;
                    }
                }

                tokio::time::sleep(Duration::from_micros(100)).await;
            }
        });

        result.push(handle);
    }

    result
}

fn bench(c: &mut Criterion, clients: usize, title: &str) {
    let rt = Runtime::new().unwrap();

    // setup the node with the configurations.
    let mut node = rt.block_on(async { setup_node().await });

    let clients = {
        let _guard = rt.enter();
        run_clients(clients)
    };

    let mut g = c.benchmark_group("primitive");
    g.bench_function(BenchmarkId::new("Handshake", title), |b| {
        b.iter(|| {
            rt.block_on(async {
                let (tx, mut rx) = establish_connection().await;
                perform_handshake(&tx, &mut rx).await;
                (tx, rx)
            })
        });
    });
    g.finish();

    let mut g = c.benchmark_group("download_256KB");
    for chunk_size in [256 * 1024, 128 * 1024, 32 * 1024, 16 * 1024] {
        let chunk_count = (256 * 1024) / chunk_size;
        g.throughput(criterion::Throughput::Bytes(MB as u64));
        g.bench_with_input(
            BenchmarkId::new(
                title,
                format!("chunk_count={chunk_count},chunk_size={chunk_size}"),
            ),
            &chunk_count,
            |b, _| {
                b.iter_batched_ref(
                    || {
                        rt.block_on(async {
                            let (tx, mut rx) = establish_connection().await;
                            perform_handshake(&tx, &mut rx).await;
                            (tx, rx)
                        })
                    },
                    |(tx, rx)| rt.block_on(io_stress_write(chunk_size, chunk_count, tx, rx)),
                    criterion::BatchSize::NumIterations(16),
                );
            },
        );
    }
    g.finish();

    let mut g = c.benchmark_group("request_n_32_byte_chunk");
    for n in [1, 2, 512, 1024, 2048] {
        g.throughput(criterion::Throughput::Elements(n));
        g.bench_with_input(BenchmarkId::new(title, format!("n={n}")), &n, |b, _| {
            b.iter_batched_ref(
                || {
                    rt.block_on(async {
                        let (tx, mut rx) = establish_connection().await;
                        perform_handshake(&tx, &mut rx).await;
                        (tx, rx)
                    })
                },
                |(tx, rx)| rt.block_on(io_stress_write(32, n as usize, tx, rx)),
                criterion::BatchSize::NumIterations(16),
            );
        });
    }
    g.finish();

    rt.block_on(async {
        for handle in clients {
            handle.abort();
        }

        println!("shutting node down.");
        node.shutdown().await;
    });
}

/// Run all the benchmarks using different node configurations.
fn run_benches(c: &mut Criterion) {
    for n in [0, 10, 100, 500, 1000] {
        bench(c, n, &format!("c={n}"));
    }
}

criterion_group! {
    name = benches;
    config = Criterion::default().sample_size(10).warm_up_time(Duration::from_secs(1));
    targets = run_benches
}

fn main() {
    if let Ok(service_id) = std::env::var("SERVICE_ID") {
        ServiceExecutor::<TestBinding>::run_service(
            service_id.parse().expect("SERVICE_ID to be a number"),
        );
        std::process::exit(0);
    }

    benches();
    criterion::Criterion::default()
        .configure_from_args()
        .final_summary();
}
