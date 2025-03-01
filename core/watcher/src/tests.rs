use std::sync::Arc;
use std::time::{Duration, SystemTime};

use axum::{Extension, Router};
use fn_sdk::header::{read_header, TransportDetail};
use futures::SinkExt;
use lightning_application::app::Application;
use lightning_blockstore::Blockstore;
use lightning_checkpointer::Checkpointer;
use lightning_committee_beacon::CommitteeBeaconComponent;
use lightning_interfaces::prelude::*;
use lightning_notifier::Notifier;
use lightning_pinger::Pinger;
use lightning_pool::PoolProvider;
use lightning_rep_collector::aggregator::ReputationAggregator;
use lightning_rpc::Rpc;
use lightning_signer::Signer;
use lightning_task_broker::TaskBroker;
use lightning_test_utils::consensus::{MockConsensus, MockForwarder};
use lightning_test_utils::e2e::{SyncBroadcaster, TestNetwork};
use lightning_test_utils::keys::EphemeralKeystore;
use lightning_topology::Topology;
use lightning_types::{Job, JobInfo, UpdateMethod};
use lightning_utils::config::TomlConfigProvider;
use lightning_utils::poll::{poll_until, PollUntilError};
use tokio::net::{TcpListener, UnixStream};
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tokio_util::codec::Framed;

use crate::Watcher;

partial_node_components!(TestFullNodeComponentsWithMockServiceExecutor {
    ApplicationInterface = Application<Self>;
    BroadcastInterface = SyncBroadcaster<Self>;
    BlockstoreInterface = Blockstore<Self>;
    CheckpointerInterface = Checkpointer<Self>;
    CommitteeBeaconInterface = CommitteeBeaconComponent<Self>;
    ConfigProviderInterface = TomlConfigProvider<Self>;
    ConsensusInterface = MockConsensus<Self>;
    ForwarderInterface = MockForwarder<Self>;
    KeystoreInterface = EphemeralKeystore<Self>;
    NotifierInterface = Notifier<Self>;
    PingerInterface = Pinger<Self>;
    PoolInterface = PoolProvider<Self>;
    ReputationAggregatorInterface = ReputationAggregator<Self>;
    RpcInterface = Rpc<Self>;
    SignerInterface = Signer<Self>;
    TopologyInterface = Topology<Self>;
    ServiceExecutorInterface = ProxyServiceExecutor;
    TaskBrokerInterface = TaskBroker<Self>;
    WatcherInterface = Watcher<Self>;
});

pub struct Counter {
    count: usize,
}

pub fn start_listening_server(state: Arc<Mutex<Counter>>) -> JoinHandle<()> {
    tokio::spawn(async move {
        let app = Router::new()
            .route("/counter", axum::routing::post(handler))
            .layer(Extension(state));
        let listener = TcpListener::bind("0.0.0.0:6006").await.unwrap();
        axum::serve::serve(listener, app.into_make_service())
            .await
            .unwrap();
    })
}

async fn handler(state: Extension<Arc<Mutex<Counter>>>) {
    state.0.lock().await.count += 1;
}

pub struct ProxyServiceExecutor {}

impl<C: NodeComponents> ServiceExecutorInterface<C> for ProxyServiceExecutor {
    type Provider = ProxyProvider;
    fn get_provider(&self) -> Self::Provider {
        ProxyProvider {}
    }
    fn run_service(_id: u32) {}
}
impl BuildGraph for ProxyServiceExecutor {
    fn build_graph() -> fdi::DependencyGraph {
        fdi::DependencyGraph::new().with_value(ProxyServiceExecutor {})
    }
}

#[derive(Clone)]
pub struct ProxyProvider {}
impl ExecutorProviderInterface for ProxyProvider {
    async fn connect(&self, service_id: types::ServiceId) -> Option<UnixStream> {
        assert_eq!(service_id, 0);

        let (client, mut server) = UnixStream::pair().ok()?;

        tokio::spawn(async move {
            let header = read_header(&mut server)
                .await
                .expect("Could not read hello frame.")
                .transport_detail;
            let mut framed = Framed::new(server, tokio_util::codec::LengthDelimitedCodec::new());

            if let TransportDetail::Task { payload, .. } = header {
                let res = reqwest::Client::new()
                    .post("http://0.0.0.0:6006/counter")
                    .send()
                    .await
                    .unwrap();

                assert_eq!(res.status(), 200);

                framed
                    .send(payload)
                    .await
                    .expect("failed to send task response");
            } else {
                unreachable!("Automated jobs use the TaskBroker interface")
            }
        });

        Some(client)
    }
}

#[tokio::test]
async fn test_watcher() -> anyhow::Result<()> {
    let epoch_time = 10000;
    lightning_test_utils::logging::setup(None);

    // Give: Some counter that will be incremented when a job
    // is executed using the task broker API.
    let state = Arc::new(Mutex::new(Counter { count: 0 }));
    let server_handle = start_listening_server(state.clone());

    // Let's wait until the server starts.
    tokio::time::sleep(Duration::from_secs(3)).await;

    // Given: a network with some nodes that will perform the jobs.
    let mut network = TestNetwork::builder()
        .with_committee_nodes::<TestFullNodeComponentsWithMockServiceExecutor>(4)
        .await
        .with_genesis_mutator(move |genesis| {
            genesis.total_intervals = 1;
            genesis.epoch_start = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64;
            genesis.epoch_time = epoch_time;
        })
        .build()
        .await
        .unwrap();
    let node = network.node(0);
    let peer1 = network.node(2);
    let peer2 = network.node(3);

    // Given: Some jobs.
    let job1 = Job {
        hash: [0; 32],
        info: JobInfo {
            frequency: 1,
            amount: 0,
            service: 0,
            arguments: vec![0; 4].into_boxed_slice(),
        },
        status: None,
        assignee: None,
    };
    let job2 = Job {
        hash: [1; 32],
        info: JobInfo {
            frequency: 1,
            amount: 0,
            service: 0,
            arguments: vec![1; 4].into_boxed_slice(),
        },
        status: None,
        assignee: None,
    };
    let job3 = Job {
        hash: [2; 32],
        info: JobInfo {
            frequency: 1,
            amount: 0,
            service: 0,
            arguments: vec![2; 4].into_boxed_slice(),
        },
        status: None,
        assignee: None,
    };

    // When: we submit these jobs for execution.
    peer1
        .execute_transaction_from_node(UpdateMethod::AddJobs { jobs: vec![job1] }, None)
        .await
        .unwrap();
    peer2
        .execute_transaction_from_node(
            UpdateMethod::AddJobs {
                jobs: vec![job2, job3],
            },
            None,
        )
        .await
        .unwrap();

    // Then: we see the jobs in state.
    poll_until(
        || async {
            let jobs = node.app_query().get_all_jobs();
            (jobs.len() == 3)
                .then_some(())
                .ok_or(PollUntilError::ConditionNotSatisfied)
        },
        Duration::from_secs(3),
        Duration::from_millis(100),
    )
    .await
    .unwrap();

    // Then: the jobs have been assigned.
    let node_assignments = node.app_query().get_job_assignments();

    // Then: the assignment account for all jobs.
    assert_eq!(
        node_assignments
            .iter()
            .map(|(_, jobs)| jobs.len())
            .reduce(|acc, e| acc + e)
            .unwrap(),
        3
    );

    // Then: the jobs are marked with the appropriate node index.
    for job in node.app_query().get_all_jobs().iter() {
        let assignee_i = node_assignments
            .binary_search_by_key(&job.assignee.unwrap(), |(i, _)| *i)
            .unwrap();
        assert!(node_assignments[assignee_i].1.contains(&job.hash));
    }

    // We wait for jobs to be updated with the status of the latest run.
    poll_until(
        || async {
            while node
                .app_query()
                .get_all_jobs()
                .into_iter()
                .any(|job| job.status.is_none())
            {
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
            Ok(())
        },
        Duration::from_secs(3),
        Duration::from_millis(100),
    )
    .await
    .unwrap();

    // Then: all jobs are executed successfully.
    assert!(node
        .app_query()
        .get_all_jobs()
        .iter()
        .all(|job| job.status.as_ref().unwrap().success));

    // Then: each job should have raised the count by 1.
    assert_eq!(state.lock().await.count, 3);

    // We sleep for a little bit to simulate a real epoch interval.
    tokio::time::sleep(Duration::from_millis(epoch_time)).await;

    // When: We switch to the new epoch.
    network
        .change_epoch_and_wait_for_complete(0, 2000, 2000)
        .await
        .unwrap();

    // Then: each job should have raised the count by 1.
    // We try a few time in case the thread of execution is too fast.
    poll_until(
        || async {
            while state.lock().await.count < 6 {
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
            Ok(())
        },
        Duration::from_secs(5),
        Duration::from_millis(500),
    )
    .await
    .unwrap();

    // Shutdown the network.
    network.shutdown().await;

    // Clean up the counter server.
    server_handle.abort();

    Ok(())
}
