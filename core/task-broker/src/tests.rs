use std::time::{Duration, SystemTime, UNIX_EPOCH};

use fdi::Provider;
use fleek_crypto::EthAddress;
use fn_sdk::header::{read_header, TransportDetail};
use futures::stream::FuturesUnordered;
use futures::{SinkExt, StreamExt};
use lightning_application::app::Application;
use lightning_application::genesis::{Genesis, GenesisNode};
use lightning_broadcast::Broadcast;
use lightning_interfaces::prelude::*;
use lightning_pool::PoolProvider;
use lightning_test_utils::json_config::JsonConfigProvider;
use lightning_test_utils::keys::EphemeralKeystore;
use lightning_topology::Topology;
use tokio::net::UnixStream;
use tokio_util::codec::Framed;
use types::NodePorts;

use crate::TaskBroker;

partial!(TestBinding {
    ConfigProviderInterface = JsonConfigProvider;
    ApplicationInterface = Application<Self>;
    KeystoreInterface = EphemeralKeystore<Self>;
    TopologyInterface = Topology<Self>;
    PoolInterface = PoolProvider<Self>;
    BroadcastInterface = Broadcast<Self>;
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

                if let TransportDetail::Task { payload } = header {
                    framed
                        .send(payload.into())
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

#[tokio::test]
async fn run_local_echo_task() -> anyhow::Result<()> {
    let dir = tempdir::TempDir::new("run_local_echo_task")?;
    let path = Genesis::default().write_to_dir(dir.path().to_path_buf().try_into().unwrap())?;
    let mut node = Node::<TestBinding>::init(
        JsonConfigProvider::default()
            .with::<Application<TestBinding>>(lightning_application::config::Config::test(path)),
    )
    .expect("failed to initialize node");
    node.start().await;

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
        .run(schema::task_broker::TaskScope::Local, request)
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

    // Initialize keystores and build genesis
    let keystores = (0..4)
        .map(|_| EphemeralKeystore::<TestBinding>::default())
        .collect::<Vec<_>>();
    let owner = EthAddress::default();
    let mut genesis = Genesis::default();
    genesis.node_info = keystores
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
                    pool: 19420 + i as u16,
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
        .collect();

    let dir = tempdir::TempDir::new("run_local_echo_task")?;
    let path = &genesis.write_to_dir(dir.path().to_path_buf().try_into().unwrap())?;

    // Initialize nodes and start them
    let mut nodes = keystores
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
                                max_idle_timeout: Duration::from_secs(5),
                                address: ([127, 0, 0, 1], 19420 + i as u16).into(),
                                http: None,
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

    // wait for topology to update
    let mut topo = nodes[0]
        .provider
        .get::<Topology<TestBinding>>()
        .get_receiver();
    if topo.borrow().is_empty() {
        topo.changed()
            .await
            .expect("failed to wait for topology update");
    }

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
        .run(schema::task_broker::TaskScope::Single, request)
        .await;

    assert!(
        response.len() == 1,
        "exactly one response should be given for single node tasks"
    );

    let response = response
        .into_iter()
        .nth(0)
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
