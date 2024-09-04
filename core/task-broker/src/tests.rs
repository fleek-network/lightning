use std::time::{Duration, SystemTime, UNIX_EPOCH};

use fdi::Provider;
use fleek_crypto::EthAddress;
use fn_sdk::header::{read_header, TransportDetail};
use futures::stream::FuturesUnordered;
use futures::{Future, SinkExt, StreamExt};
use lightning_application::app::Application;
use lightning_interfaces::prelude::*;
use lightning_interfaces::types::{Genesis, GenesisNode};
use lightning_interfaces::TaskError;
use lightning_pool::PoolProvider;
use lightning_test_utils::json_config::JsonConfigProvider;
use lightning_test_utils::keys::EphemeralKeystore;
use lightning_topology::Topology;
use tempdir::TempDir;
use tokio::net::UnixStream;
use tokio_util::codec::Framed;
use types::NodePorts;

use crate::{TaskBroker, TaskBrokerConfig};

partial!(TestBinding {
    ConfigProviderInterface = JsonConfigProvider;
    ApplicationInterface = Application<Self>;
    KeystoreInterface = EphemeralKeystore<Self>;
    TopologyInterface = Topology<Self>;
    PoolInterface = PoolProvider<Self>;
    ServiceExecutorInterface = EchoServiceExecutor;
    TaskBrokerInterface = TaskBroker<Self>;
});

pub struct EchoServiceExecutor {}
impl<C: Collection> ServiceExecutorInterface<C> for EchoServiceExecutor {
    type Provider = EchoProvider;
    fn get_provider(&self) -> Self::Provider {
        EchoProvider {}
    }
    fn run_service(_id: u32) {}
}
impl BuildGraph for EchoServiceExecutor {
    fn build_graph() -> fdi::DependencyGraph {
        fdi::DependencyGraph::new().with_value(EchoServiceExecutor {})
    }
}
#[derive(Clone)]
pub struct EchoProvider {}
impl ExecutorProviderInterface for EchoProvider {
    async fn connect(&self, service_id: types::ServiceId) -> Option<UnixStream> {
        if service_id == 0 {
            let (client, mut server) = UnixStream::pair().ok()?;

            tokio::spawn(async move {
                let header = read_header(&mut server)
                    .await
                    .expect("Could not read hello frame.")
                    .transport_detail;

                let mut framed =
                    Framed::new(server, tokio_util::codec::LengthDelimitedCodec::new());

                if let TransportDetail::Task { payload, .. } = header {
                    framed
                        .send(payload)
                        .await
                        .expect("failed to send task response");
                } else {
                    while let Some(Ok(bytes)) = framed.next().await {
                        framed
                            .send(bytes.into())
                            .await
                            .expect("failed to send response");
                    }
                }
            });

            Some(client)
        } else {
            None
        }
    }
}

#[track_caller]
fn build_cluster(
    n: usize,
) -> impl Future<Output = anyhow::Result<(TempDir, Vec<Node<TestBinding>>)>> {
    // Use the test's line number to create a temp dir and port offset.
    // Majority of tests are more lines of code than nodes needed.
    let line = std::panic::Location::caller().line() as u16;
    let name = format!("task-broker-test-{line}");
    let port_start = 13000 + line;
    let owner = EthAddress::default();
    async move {
        let dir = tempdir::TempDir::new(&name)?;

        // Initialize keystores
        let keystores = (0..n)
            .map(|_| EphemeralKeystore::<TestBinding>::default())
            .collect::<Vec<_>>();

        // Build genesis
        let genesis = Genesis {
            node_info: keystores
                .iter()
                .enumerate()
                .map(|(i, k)| {
                    GenesisNode::new(
                        owner,
                        k.get_ed25519_pk(),
                        [127, 0, 0, 1].into(),
                        k.get_bls_pk(),
                        [127, 0, 0, 1].into(),
                        k.get_ed25519_pk(),
                        NodePorts {
                            pool: port_start + i as u16,
                            ..Default::default()
                        },
                        Some(types::Staking {
                            staked: 420u64.into(),
                            stake_locked_until: 0,
                            locked: 0u64.into(),
                            locked_until: 0,
                        }),
                        true,
                    )
                })
                .collect(),
            ..Default::default()
        };

        // Write genesis
        let path = &genesis.write_to_dir(dir.path().to_path_buf().try_into().unwrap())?;

        // Initialize nodes and start them
        let nodes = keystores
            .into_iter()
            .enumerate()
            .map(|(i, keystore)| async move {
                let node = Node::<TestBinding>::init_with_provider(
                    Provider::default()
                        .with(
                            JsonConfigProvider::default()
                                .with::<Application<TestBinding>>(
                                    lightning_application::config::Config::test(path.clone()),
                                )
                                .with::<PoolProvider<TestBinding>>(lightning_pool::Config {
                                    max_idle_timeout: Duration::from_millis(100),
                                    address: ([127, 0, 0, 1], port_start + i as u16).into(),
                                    http: None,
                                })
                                .with::<TaskBroker<TestBinding>>(TaskBrokerConfig {
                                    connect_timeout: Duration::from_secs(5),
                                    ..Default::default()
                                }),
                        )
                        .with(keystore),
                )
                .expect("failed to initialize node");
                node.start().await;
                node
            })
            .collect::<FuturesUnordered<_>>()
            .collect::<Vec<_>>()
            .await;

        // Wait for topology to init
        let mut topo = nodes[0]
            .provider
            .get::<Topology<TestBinding>>()
            .get_receiver();
        if topo.borrow().is_empty() {
            topo.changed()
                .await
                .expect("failed to wait for topology update");
        }

        Ok((dir, nodes))
    }
}

#[tokio::test]
async fn run_local_echo_task() -> anyhow::Result<()> {
    let (_, mut nodes) = build_cluster(1).await?;
    let mut node = nodes.remove(0);
    let broker = node.provider.get::<TaskBroker<TestBinding>>();

    const PAYLOAD: &[u8] = b"hello world";
    let request = schema::task_broker::TaskRequest {
        service: 0,
        timestamp: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        payload: PAYLOAD.into(),
    };
    let digest = request.to_digest();

    let response = broker
        .run(0, schema::task_broker::TaskScope::Local, request)
        .await;

    assert!(
        response.len() == 1,
        "exactly one response should be given for local tasks"
    );

    let response = response[0].clone().expect("task to succeed");

    assert_eq!(response.payload, bytes::Bytes::from(PAYLOAD));
    assert_eq!(response.request, digest);

    node.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn run_single_echo_task() -> anyhow::Result<()> {
    lightning_test_utils::logging::setup();
    let (_, mut nodes) = build_cluster(4).await?;

    let broker = nodes[0].provider.get::<TaskBroker<TestBinding>>();

    const PAYLOAD: &[u8] = b"hello world";
    let request = schema::task_broker::TaskRequest {
        service: 0,
        timestamp: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        payload: PAYLOAD.into(),
    };
    let digest = request.to_digest();

    let response = broker
        .run(0, schema::task_broker::TaskScope::Single, request)
        .await;

    assert!(
        response.len() == 1,
        "exactly one response should be given for single node tasks"
    );

    let response = response
        .into_iter()
        .next()
        .unwrap()
        .expect("task to succeed");

    assert_eq!(response.payload, PAYLOAD);
    assert_eq!(response.request, digest);

    // Shutdown all nodes
    nodes
        .iter_mut()
        .map(|n| n.shutdown())
        .collect::<FuturesUnordered<_>>()
        .collect::<()>()
        .await;

    Ok(())
}

#[tokio::test]
async fn run_single_echo_task_with_fallback() -> anyhow::Result<()> {
    lightning_test_utils::logging::setup();
    let (_, mut nodes) = build_cluster(4).await?;

    // shutdown all nodes except 2; one to perform the request,
    // and another to be the fallback node.
    nodes
        .iter_mut()
        .skip(2)
        .map(|n| n.shutdown())
        .collect::<FuturesUnordered<_>>()
        .collect::<()>()
        .await;

    // Run the test a few times just for a bit of fuzz given the rng used for node selection.
    for i in 0..8 {
        let broker = nodes[0].provider.get::<TaskBroker<TestBinding>>();

        const PAYLOAD: &[u8] = b"hello world";
        let request = schema::task_broker::TaskRequest {
            service: 0,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            payload: PAYLOAD.into(),
        };
        let digest = request.to_digest();

        let response = broker
            .run(0, schema::task_broker::TaskScope::Single, request)
            .await;

        assert!(
            response.len() == 1,
            "exactly one response should be given for single node tasks"
        );

        let response = response
            .into_iter()
            .next()
            .unwrap()
            .expect("task to succeed");

        assert_eq!(response.payload, PAYLOAD);
        assert_eq!(response.request, digest);

        println!("finished task {i}");
    }

    // Shutdown the last 2 running nodes
    nodes.get_mut(0).unwrap().shutdown().await;
    nodes.get_mut(1).unwrap().shutdown().await;

    Ok(())
}

#[tokio::test]
async fn run_cluster_echo_task() -> anyhow::Result<()> {
    lightning_test_utils::logging::setup();
    let (_, mut nodes) = build_cluster(8).await?;

    let broker = nodes[0].provider.get::<TaskBroker<TestBinding>>();

    const PAYLOAD: &[u8] = b"hello world";
    let request = schema::task_broker::TaskRequest {
        service: 0,
        timestamp: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        payload: PAYLOAD.into(),
    };
    let digest = request.to_digest();

    let responses = broker
        .run(0, schema::task_broker::TaskScope::Cluster, request)
        .await;

    // Ensure at least 2/3 of nodes had a response
    assert!(responses.len() >= (2. * (nodes.len() - 1) as f32 / 3.).ceil() as usize);

    for response in responses {
        let response = response.expect("task should succeed");
        assert_eq!(response.payload, PAYLOAD);
        assert_eq!(response.request, digest);
    }

    // Shutdown all nodes
    nodes
        .iter_mut()
        .map(|n| n.shutdown())
        .collect::<FuturesUnordered<_>>()
        .collect::<()>()
        .await;

    Ok(())
}

#[tokio::test]
async fn run_cluster_echo_task_1_offline_of_8() -> anyhow::Result<()> {
    lightning_test_utils::logging::setup();
    let (_, mut nodes) = build_cluster(8).await?;

    // shutdown a node
    nodes.remove(0).shutdown().await;

    let broker = nodes[1].provider.get::<TaskBroker<TestBinding>>();

    const PAYLOAD: &[u8] = b"hello world";
    let request = schema::task_broker::TaskRequest {
        service: 0,
        timestamp: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        payload: PAYLOAD.into(),
    };
    let digest = request.to_digest();

    let responses = broker
        .run(0, schema::task_broker::TaskScope::Cluster, request)
        .await;

    // Ensure at least 2/3 of nodes had a response
    assert!(responses.len() >= (2. * nodes.len() as f32 / 3.).ceil() as usize);

    for response in responses {
        match response {
            Ok(response) => {
                assert_eq!(response.payload, PAYLOAD);
                assert_eq!(response.request, digest);
            },
            Err(e) => {
                if !matches!(e, TaskError::PeerDisconnect | TaskError::Timeout) {
                    panic!("unexpected error")
                }
            },
        }
    }

    // Shutdown all nodes
    nodes
        .iter_mut()
        .map(|n| n.shutdown())
        .collect::<FuturesUnordered<_>>()
        .collect::<()>()
        .await;

    Ok(())
}

#[tokio::test]
async fn run_cluster_echo_task_7_offline_of_8_should_fail() -> anyhow::Result<()> {
    lightning_test_utils::logging::setup();
    let (_, mut nodes) = build_cluster(8).await?;

    // shutdown all nodes but one
    nodes
        .iter_mut()
        .skip(1)
        .map(Node::shutdown)
        .collect::<FuturesUnordered<_>>()
        .collect::<()>()
        .await;

    let broker = nodes[0].provider.get::<TaskBroker<TestBinding>>();

    const PAYLOAD: &[u8] = b"hello world";
    let request = schema::task_broker::TaskRequest {
        service: 0,
        timestamp: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        payload: PAYLOAD.into(),
    };
    // let digest = request.to_digest();

    let responses = broker
        .run(0, schema::task_broker::TaskScope::Cluster, request)
        .await;

    for response in responses {
        let response = response.expect_err("only errors should happen");
        assert!(
            response == TaskError::Connect,
            "expected connection error, not {response}"
        );
    }

    // Shutdown the last node
    nodes.get_mut(0).unwrap().shutdown().await;

    Ok(())
}
