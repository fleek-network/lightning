use std::sync::Arc;
use std::time::{Duration, SystemTime};

use axum::{Extension, Router};
use fleek_crypto::NodePublicKey;
use fn_sdk::header::{read_header, TransportDetail};
use futures::SinkExt;
use lightning_application::app::Application;
use lightning_archive::archive::Archive;
use lightning_blockstore::blockstore::Blockstore;
use lightning_blockstore_server::BlockstoreServer;
use lightning_broadcast::Broadcast;
use lightning_checkpointer::Checkpointer;
use lightning_committee_beacon::CommitteeBeaconComponent;
use lightning_e2e::swarm::Swarm;
use lightning_forwarder::Forwarder;
use lightning_interfaces::fdi::BuildGraph;
use lightning_interfaces::types::{Job, JobInfo, UpdateMethod};
use lightning_interfaces::{
    fdi,
    partial_node_components,
    types,
    ExecutorProviderInterface,
    NodeComponents,
    ServiceExecutorInterface,
};
use lightning_notifier::Notifier;
use lightning_pinger::Pinger;
use lightning_pool::PoolProvider;
use lightning_rep_collector::ReputationAggregator;
use lightning_rpc::Rpc;
use lightning_signer::Signer;
use lightning_task_broker::TaskBroker;
use lightning_test_utils::logging;
use lightning_topology::Topology;
use lightning_utils::config::TomlConfigProvider;
use lightning_utils::poll::{poll_until, PollUntilError};
use lightning_watcher::Watcher;
use tempfile::tempdir;
use tokio::net::{TcpListener, UnixStream};
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tokio_util::codec::Framed;

partial_node_components!(TestFullNodeComponentsWithMockServiceExecutor {
    ForwarderInterface = Forwarder<Self>;
    ConsensusInterface = Consensus<Self>;
    CheckpointerInterface = Checkpointer<Self>;
    ConfigProviderInterface = TomlConfigProvider<Self>;
    CommitteeBeaconInterface = CommitteeBeaconComponent<Self>;
    ApplicationInterface = Application<Self>;
    BlockstoreInterface = Blockstore<Self>;
    BlockstoreServerInterface = BlockstoreServer<Self>;
    SyncronizerInterface = Syncronizer<Self>;
    BroadcastInterface = Broadcast<Self>;
    TopologyInterface = Topology<Self>;
    ArchiveInterface = Archive<Self>;
    NotifierInterface = Notifier<Self>;
    ReputationAggregatorInterface = ReputationAggregator<Self>;
    ResolverInterface = Resolver<Self>;
    RpcInterface = Rpc<Self>;
    ServiceExecutorInterface = ProxyServiceExecutor;
    TaskBrokerInterface = TaskBroker<Self>;
    KeystoreInterface = Keystore<Self>;
    SignerInterface = Signer<Self>;
    PoolInterface = PoolProvider<Self>;
    PingerInterface = Pinger<Self>;
    WatcherInterface = Watcher<Self>;
    DeliveryAcknowledgmentAggregatorInterface = lightning_interfaces::_hacks::Blanket;
});
use lightning_consensus::Consensus;
use lightning_interfaces::SyncQueryRunnerInterface;
use lightning_keystore::Keystore;
use lightning_resolver::Resolver;
use lightning_syncronizer::Syncronizer;

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
async fn test_watcher() {
    logging::setup(None);

    let epoch_time = 10000;

    // Give: Some counter that will be incremented when a job
    // is executed using the task broker API.
    let state = Arc::new(Mutex::new(Counter { count: 0 }));
    let server_handle = start_listening_server(state.clone());

    // Given: a swarm with a given epoch time and total intervals of 1.
    // Given: this will execute 1 run of jobs per epoch.
    let temp_dir = tempdir().unwrap();
    let mut swarm = Swarm::<TestFullNodeComponentsWithMockServiceExecutor>::builder()
        .with_directory(temp_dir.path().to_path_buf().try_into().unwrap())
        .with_min_port(11000)
        .with_num_nodes(4)
        // We need to include enough time in this epoch time for the nodes to start up, or else it
        // begins the epoch change immediately when they do. We can even get into a situation where
        // another epoch change starts quickly after that, causing our expectation of epoch = 1
        // below to fail.
        .with_epoch_time(epoch_time)
        .with_commit_phase_time(3000)
        .with_reveal_phase_time(3000)
        .with_epoch_start(
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
        )
        .with_syncronizer_delta(Duration::from_secs(5))
        .with_total_intervals(1)
        .build::<TestFullNodeComponentsWithMockServiceExecutor>();
    swarm.launch().await.unwrap();

    // Wait for RPC to be ready.
    swarm.wait_for_rpc_ready().await;

    // Given: some nodes from the swarm.
    let pubkeys: Vec<NodePublicKey> = swarm.get_ports().keys().cloned().collect();
    let peer1 = pubkeys[0];
    let peer2 = pubkeys[1];
    let node = pubkeys[2];

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
    swarm
        .execute_transaction_from_node(&peer1, UpdateMethod::AddJobs { jobs: vec![job1] })
        .await
        .unwrap();
    swarm
        .execute_transaction_from_node(
            &peer2,
            UpdateMethod::AddJobs {
                jobs: vec![job2, job3],
            },
        )
        .await
        .unwrap();

    // Then: we see the jobs in state.
    let query_runner = swarm.get_query_runner(&node).unwrap();
    poll_until(
        || async {
            let jobs = query_runner.get_all_jobs();
            (jobs.len() == 3)
                .then_some(())
                .ok_or(PollUntilError::ConditionNotSatisfied)
        },
        Duration::from_secs(3),
        Duration::from_millis(10),
    )
    .await
    .unwrap();

    // Then: the jobs have been assigned.
    let node_assignments = query_runner.get_job_assignments();

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
    for job in query_runner.get_all_jobs().iter() {
        let assignee_i = node_assignments
            .binary_search_by_key(&job.assignee.unwrap(), |(i, _)| *i)
            .unwrap();
        assert!(node_assignments[assignee_i].1.contains(&job.hash));
    }

    // We wait for jobs to be updated with the status of the latest run.
    poll_until(
        || async {
            while query_runner
                .get_all_jobs()
                .into_iter()
                .any(|job| job.status.is_none())
            {
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
            Ok(())
        },
        Duration::from_secs(3),
        Duration::from_millis(10),
    )
    .await
    .unwrap();

    // Then: all jobs are executed successfully.
    assert!(query_runner
        .get_all_jobs()
        .iter()
        .all(|job| job.status.as_ref().unwrap().success));

    // Then: each job should have raised the count by 1.
    assert_eq!(state.lock().await.count, 3);

    // When: we wait for an entire epoch.
    swarm
        .wait_for_epoch_change(1, Duration::from_secs(60))
        .await
        .unwrap();
    swarm
        .wait_for_epoch_change(2, Duration::from_secs(60))
        .await
        .unwrap();

    // Then: each job should have raised the count by 1 in the last epoch.
    // We try a few time in case the thread of execution is too fast.
    poll_until(
        || async {
            while state.lock().await.count < 6 {
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
            Ok(())
        },
        Duration::from_secs(5),
        Duration::from_millis(100),
    )
    .await
    .unwrap();

    // Clean up the swarm.
    swarm.shutdown().await;

    // Clean up the counter server.
    server_handle.abort();
}
