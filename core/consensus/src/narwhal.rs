use fleek_crypto::{ConsensusPublicKey, NodePublicKey};
use lightning_interfaces::SyncQueryRunnerInterface;
use mysten_metrics::RegistryService;
use narwhal_config::{Committee, Parameters, WorkerCache};
use narwhal_crypto::traits::KeyPair as _;
use narwhal_crypto::{KeyPair, NetworkKeyPair};
use narwhal_network::client::NetworkClient;
use narwhal_node::primary_node::PrimaryNode;
use narwhal_node::worker_node::WorkerNode;
use narwhal_node::NodeStorage;
use sui_protocol_config::{Chain, ProtocolConfig, ProtocolVersion};
use tokio::sync::mpsc::Sender;
use tokio::sync::Mutex;
use tokio::time::Instant;
use tracing::{debug, error, trace};

use crate::execution::state::{Execution, FilteredConsensusOutput};
use crate::validator::Validator;

// Copyright 2022-2023 Fleek Network
// SPDX-License-Identifier: Apache-2.0, MIT

/// Manages running the narwhal and bullshark as a service.
pub struct NarwhalService {
    node_public_key: NodePublicKey,
    consensus_public_key: ConsensusPublicKey,
    arguments: NarwhalArgs,
    store: NodeStorage,
    primary: PrimaryNode,
    worker_node: WorkerNode,
    committee: Committee,
    worker_cache: WorkerCache,
    status: Mutex<Status>,
    protocol_config: ProtocolConfig,
}

/// Arguments used to run a consensus service.
pub struct NarwhalArgs {
    pub primary_keypair: KeyPair,
    pub primary_network_keypair: NetworkKeyPair,
    pub worker_keypair: NetworkKeyPair,
    pub registry_service: RegistryService,
}

#[derive(PartialEq)]
enum Status {
    Running,
    Stopped,
}

impl NarwhalService {
    /// Create a new narwhal service using the provided arguments.
    pub fn new(
        node_public_key: NodePublicKey,
        consensus_public_key: ConsensusPublicKey,
        arguments: NarwhalArgs,
        store: NodeStorage,
        committee: Committee,
        worker_cache: WorkerCache,
    ) -> Self {
        let protocol_config =
            ProtocolConfig::get_for_version_if_supported(ProtocolVersion::new(20), Chain::Unknown)
                .unwrap();
        // Todo(dalton): Create our own protocol default paramaters
        let primary = PrimaryNode::new(Parameters::default(), arguments.registry_service.clone());

        // Sui intertwined their protocol config with there narwhal library with a recent update,
        // which is why we are bringing ProtocolConfig in and passing it to the worker.
        // Out of the 100s of parameters and feature flags that are on this struct, the only one
        // that applies to narwhal is the feature flag narwhal_versioned_metadata
        // which is set to true starting at version 12 of ProtocolConfig. None of the other
        // parameters or feature flags will have any effect on narwhal or how it runs. The versioned
        // metadata feature seems to be useful and definitly future proofs our metadata so I
        // am going to set it to version 12 for now but we might want to consider a fork at this
        // point, if they are going to intertwine their narwhal implementation with Sui
        // anymore.
        let worker_node = WorkerNode::new(
            0,
            protocol_config.clone(),
            Parameters::default(),
            arguments.registry_service.clone(),
        );

        Self {
            node_public_key,
            consensus_public_key,
            arguments,
            store,
            primary,
            worker_node,
            committee,
            worker_cache,
            status: Mutex::new(Status::Stopped),
            protocol_config,
        }
    }

    /// Start the narwhal process by starting the Narwhal's primary and worker.
    ///
    /// # Panics
    ///
    /// This function panics if it can not start either the Primary or the Worker.
    pub async fn start<Q: SyncQueryRunnerInterface>(
        &self,
        consensus_output_tx: Sender<FilteredConsensusOutput>,
        query_runner: Q,
    ) {
        let mut status = self.status.lock().await;
        if *status == Status::Running {
            error!("NarwhalService is already running.");
            return;
        }

        let name = self.arguments.primary_keypair.public().clone();

        let epoch = self.committee.epoch();
        debug!("Starting NarwhalService for epoch {epoch}");

        // Create the network client the primary and worker use to communicate.
        let network_client =
            NetworkClient::new_from_keypair(&self.arguments.primary_network_keypair);

        // Start the narwhal primary.
        debug!("Starting the Narwhal Primary...");
        let execution_state = Execution::new(consensus_output_tx.clone(), query_runner.clone());
        if let Err(e) = self
            .primary
            .start(
                self.arguments.primary_keypair.copy(),
                self.arguments.primary_network_keypair.copy(),
                self.committee.clone(),
                self.protocol_config.clone(),
                self.worker_cache.clone(),
                network_client.clone(),
                &self.store,
                execution_state,
            )
            .await
        {
            panic!("Unable to start Narwhal Primary: {e:?}");
        }

        // Start the narwhal worker.
        debug!("Starting the Narwhal Worker...");
        if let Err(e) = self
            .worker_node
            .start(
                name.clone(),
                self.arguments.worker_keypair.copy(),
                self.committee.clone(),
                self.worker_cache.clone(),
                network_client.clone(),
                &self.store,
                Validator::new(self.node_public_key, self.consensus_public_key),
                None,
            )
            .await
        {
            panic!("Unable to start Narwhal Worker: {e:?}");
        }

        *status = Status::Running;
    }

    /// Shutdown the primary and the worker and waits until nodes have shutdown.
    pub async fn shutdown(&self) {
        let mut status = self.status.lock().await;
        if *status == Status::Stopped {
            error!("Narwhal shutdown was called but node is not running.");
            return;
        }

        let now = Instant::now();
        let epoch = self.committee.epoch();
        trace!("Shutting down Narwhal epoch {epoch:?}");

        self.worker_node.shutdown().await;
        self.primary.shutdown().await;

        debug!(
            "Narwhal shutdown for epoch {:?} is complete - took {} seconds",
            epoch,
            now.elapsed().as_secs_f64()
        );

        *status = Status::Stopped;
    }
}

impl Clone for NarwhalArgs {
    fn clone(&self) -> Self {
        Self {
            primary_keypair: self.primary_keypair.copy(),
            primary_network_keypair: self.primary_network_keypair.copy(),
            worker_keypair: self.worker_keypair.copy(),
            registry_service: self.registry_service.clone(),
        }
    }
}
