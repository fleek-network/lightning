use std::sync::Arc;

use fastcrypto::traits::KeyPair as _;
use multiaddr::Multiaddr;
use mysten_metrics::RegistryService;
use narwhal_config::{Committee, Parameters, WorkerCache};
use narwhal_crypto::{KeyPair, NetworkKeyPair};
use narwhal_executor::ExecutionState;
use narwhal_network::client::NetworkClient;
use narwhal_node::{primary_node::PrimaryNode, worker_node::WorkerNode, NodeStorage};
use sui_protocol_config::{Chain, ProtocolConfig, ProtocolVersion};
use tokio::{sync::Mutex, time::Instant};

use crate::validator::Validator;

// Copyright 2022-2023 Fleek Network
// SPDX-License-Identifier: Apache-2.0, MIT
// use crate::keys::LoadOrCreate;
// use crate::{config::ConsensusConfig, validator::Validator};

/// Maximum number of times we retry to start the primary or the worker, before we panic.
const MAX_RETRIES: u32 = 2;

/// Manages running the narwhal and bullshark as a service.
pub struct NarwhalService {
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
    pub primary_address: Multiaddr,
    pub worker_address: Multiaddr,
    pub worker_mempool: Multiaddr,
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
        arguments: NarwhalArgs,
        store: NodeStorage,
        committee: Committee,
        worker_cache: WorkerCache,
    ) -> Self {
        let protocol_config =
            ProtocolConfig::get_for_version_if_supported(ProtocolVersion::new(12), Chain::Unknown)
                .unwrap();
        // Todo(dalton): Create our own protocol default paramaters
        let primary = PrimaryNode::new(
            Parameters::default(),
            true,
            arguments.registry_service.clone(),
        );

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
    pub async fn start<State>(&self, state: State)
    where
        State: ExecutionState + Send + Sync + 'static,
    {
        let mut status = self.status.lock().await;
        if *status == Status::Running {
            println!("NarwhalService is already running.");
            return;
        }

        let name = self.arguments.primary_keypair.public().clone();
        let execution_state = Arc::new(state);

        let epoch = self.committee.epoch();
        println!("Starting NarwhalService for epoch {epoch}");

        // create the network client the primary and worker use to communicate
        let network_client =
            NetworkClient::new_from_keypair(&self.arguments.primary_network_keypair);

        let mut running = false;
        for i in 0..MAX_RETRIES {
            println!("Trying to start the Narwhal Primary...");
            if i > 0 {
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            }

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
                    execution_state.clone(),
                )
                .await
            {
                println!("Unable to start Narwhal Primary: {e:?}");
            } else {
                running = true;
                break;
            }
        }
        if !running {
            panic!("Failed to start the Narwhal Primary after {MAX_RETRIES} tries",);
        }

        let mut running = false;
        for i in 0..MAX_RETRIES {
            println!("Trying to start the Narwhal Worker...");
            if i > 0 {
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            }

            if let Err(e) = self
                .worker_node
                .start(
                    name.clone(),
                    self.arguments.worker_keypair.copy(),
                    self.committee.clone(),
                    self.worker_cache.clone(),
                    network_client.clone(),
                    &self.store,
                    Validator::new(),
                    None,
                )
                .await
            {
                println!("Unable to start Narwhal Worker: {e:?}");
            } else {
                running = true;
                break;
            }
        }

        if !running {
            panic!("Failed to start the Narwhal Worker after {MAX_RETRIES} tries",);
        }

        *status = Status::Running;
    }

    /// Shutdown the primary and the worker and waits until nodes have shutdown.
    pub async fn shutdown(&self) {
        let mut status = self.status.lock().await;
        if *status == Status::Stopped {
            println!("Narwhal shutdown was called but node is not running.");
            return;
        }

        let now = Instant::now();
        let epoch = self.committee.epoch();
        println!("Shutting down Narwhal epoch {epoch:?}");

        self.worker_node.shutdown().await;
        self.primary.shutdown().await;

        println!(
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
            primary_address: self.primary_address.clone(),
            worker_address: self.worker_address.clone(),
            worker_mempool: self.worker_mempool.clone(),
            registry_service: self.registry_service.clone(),
        }
    }
}
