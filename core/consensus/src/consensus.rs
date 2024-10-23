use std::sync::Arc;

use derive_more::{From, IsVariant, TryInto};
use fastcrypto::bls12381::min_sig::BLS12381PrivateKey;
use fastcrypto::ed25519::Ed25519PrivateKey;
use fleek_crypto::SecretKey;
use lightning_interfaces::prelude::*;
use lightning_interfaces::types::Topic;
use lightning_interfaces::Events;
use mysten_metrics::RegistryService;
use narwhal_crypto::traits::KeyPair as _;
use narwhal_crypto::{KeyPair, NetworkKeyPair};
use prometheus::Registry;
use ready::tokio::TokioReadyWaiter;
use ready::ReadyWaiter;
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, oneshot, Notify};
use tokio::{pin, select};
use typed_store::DBMetrics;

use crate::config::ConsensusConfig;
use crate::epoch_state::EpochState;
use crate::execution::parcel::{AuthenticStampedParcel, CommitteeAttestation, Digest};
use crate::narwhal::NarwhalArgs;

pub type ConsensusReadyWaiter = TokioReadyWaiter<()>;

pub struct Consensus<C: NodeComponents> {
    /// Inner state of the consensus
    #[allow(clippy::type_complexity)]
    epoch_state: Option<
        EpochState<
            c![C::ApplicationInterface::SyncExecutor],
            c![C::BroadcastInterface::PubSub<PubSubMsg>],
            c![C::NotifierInterface::Emitter],
        >,
    >,
    /// To send the event sender to the execution worker.
    event_tx_tx: Option<oneshot::Sender<Events>>,
    /// Timestamp of the narwhal certificate that caused an epoch change
    /// is sent through this channel to notify that epoch chould change.
    reconfigure_notify: Arc<Notify>,
    /// To notify the epoch state when consensus is shutting down
    shutdown_notify_epoch_state: Arc<Notify>,
    /// To notify the epoch state when consensus is ready
    ready: ConsensusReadyWaiter,
}

impl<C: NodeComponents> Consensus<C> {
    /// Start the system, should not do anything if the system is already
    /// started.
    fn start(
        &mut self,
        app_query: fdi::Cloned<c![C::ApplicationInterface::SyncExecutor]>,
        fdi::Cloned(waiter): fdi::Cloned<ShutdownWaiter>,
    ) {
        let reconfigure_notify = self.reconfigure_notify.clone();
        let shutdown_notify_epoch_state = self.shutdown_notify_epoch_state.clone();

        let mut epoch_state = self
            .epoch_state
            .take()
            .expect("Consensus was tried to start before initialization");

        let panic_waiter = waiter.clone();
        let app_query = app_query.clone();
        spawn!(
            async move {
                app_query.wait_for_genesis().await;

                let execution_worker =
                    epoch_state.spawn_execution_worker(reconfigure_notify.clone());
                epoch_state.start_current_epoch().await;

                let shutdown_future = waiter.wait_for_shutdown();
                pin!(shutdown_future);

                loop {
                    let reconfigure_future = reconfigure_notify.notified();

                    select! {
                        biased;
                        _ = &mut shutdown_future => {
                            if let Some(consensus) = epoch_state.consensus.take() {
                                consensus.shutdown().await;
                            }
                            execution_worker.shutdown().await;
                            epoch_state.shutdown();
                            break
                        }
                        _ = reconfigure_future => {
                            epoch_state.move_to_next_epoch().await;
                            continue
                        }
                    }
                }

                // Notify the epoch state that it is time to shutdown.
                shutdown_notify_epoch_state.notify_waiters();
            },
            "CONSENSUS",
            crucial(panic_waiter)
        );
    }
}

impl<C: NodeComponents> ConfigConsumer for Consensus<C> {
    const KEY: &'static str = "consensus";
    type Config = ConsensusConfig;
}

impl<C: NodeComponents> BuildGraph for Consensus<C> {
    fn build_graph() -> fdi::DependencyGraph {
        fdi::DependencyGraph::new().with(
            Self::init
                .with_event_handler("_post", Self::post_init)
                .with_event_handler("start", Self::start),
        )
    }
}

impl<C: NodeComponents> ConsensusInterface<C> for Consensus<C> {
    type Certificate = PubSubMsg;

    type ReadyState = ();

    async fn wait_for_ready(&self) -> Self::ReadyState {
        self.ready.wait().await
    }
}

impl<C: NodeComponents> Consensus<C> {
    /// Create a new consensus service with the provided config and executor.
    fn init(
        config_provider: &C::ConfigProviderInterface,
        keystore: &C::KeystoreInterface,
        signer: &C::SignerInterface,
        app: &C::ApplicationInterface,
        broadcast: &C::BroadcastInterface,
        notifier: &C::NotifierInterface,
    ) -> anyhow::Result<Self> {
        let config = config_provider.get::<Self>();
        let executor = app.transaction_executor();
        let query_runner = app.sync_query();
        let pubsub = broadcast.get_pubsub(Topic::Consensus);

        // Spawn the registry for narwhal
        let registry = Registry::new();
        // Init the metrics for narwhal
        DBMetrics::init(&registry);

        let (consensus_sk, primary_sk) = (keystore.get_bls_sk(), keystore.get_ed25519_sk());
        let (consensus_pk, primary_pk) = (consensus_sk.to_pk(), primary_sk.to_pk());
        let reconfigure_notify = Arc::new(Notify::new());
        let networking_keypair = NetworkKeyPair::from(Ed25519PrivateKey::from(&primary_sk));
        let primary_keypair = KeyPair::from(BLS12381PrivateKey::from(&consensus_sk));
        let narwhal_args = NarwhalArgs {
            primary_keypair,
            primary_network_keypair: networking_keypair.copy(),
            worker_keypair: networking_keypair,
            registry_service: RegistryService::new(registry),
        };

        // Todo(dalton): Figure out better default channel size
        let (consensus_output_tx, consensus_output_rx) = mpsc::channel(1000);
        let (event_tx_tx, event_tx_rx) = oneshot::channel();

        let shutdown_notify_epoch_state = Arc::new(Notify::new());

        let ready = ConsensusReadyWaiter::new();

        let epoch_state = EpochState::new(
            executor,
            primary_pk,
            consensus_pk,
            query_runner,
            narwhal_args,
            config.store_path,
            signer.get_socket(),
            notifier.get_emitter(),
            pubsub,
            consensus_output_tx,
            consensus_output_rx,
            event_tx_rx,
            shutdown_notify_epoch_state.clone(),
            ready.clone(),
        );

        Ok(Self {
            epoch_state: Some(epoch_state),
            event_tx_tx: Some(event_tx_tx),
            reconfigure_notify,
            shutdown_notify_epoch_state,
            ready,
        })
    }

    fn post_init(&mut self, rpc: &C::RpcInterface) {
        self.event_tx_tx
            .take()
            .expect("Tried to start consensus before initialization")
            .send(rpc.event_tx())
            .expect("Failed to send events sender to execution worker");
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, IsVariant, From, TryInto)]
pub enum PubSubMsg {
    Transactions(AuthenticStampedParcel),
    Attestation(CommitteeAttestation),
    RequestTransactions(Digest),
}

impl AutoImplSerde for PubSubMsg {}
