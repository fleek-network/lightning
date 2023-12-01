use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use anyhow::Result;
use bytes::Bytes;
use futures::stream::FuturesUnordered;
use futures::StreamExt;
use lightning_interfaces::infu_collection::{c, Collection};
use lightning_interfaces::types::NodeIndex;
use lightning_interfaces::{ApplicationInterface, SyncQueryRunnerInterface};
use tokio::sync::mpsc::Receiver;
use tokio::sync::{oneshot, Notify};
use tokio::task::JoinHandle;

use crate::pool::{Pool, ValueRespond};
use crate::table::bucket::MAX_BUCKETS;
use crate::table::manager::Manager;
use crate::table::{bucket, distance, Event, NodeInfo};

pub type TableKey = [u8; 32];

pub struct Server<C: Collection, M, P> {
    /// Our node index.
    us: TableKey,
    /// Queue on incoming requests.
    request_queue: Receiver<Request>,
    /// Queue on incoming events.
    event_queue: Receiver<Event>,
    /// Local store.
    store: HashMap<TableKey, Bytes>,
    /// Table manager.
    manager: M,
    /// Pool client.
    pool: P,
    /// Sync query.
    sync_query: c![C::ApplicationInterface::SyncExecutor],
    /// Bootstrap-related tasks.
    bootstrap_tasks: FuturesUnordered<JoinHandle<Result<Vec<NodeIndex>>>>,
    /// State of bootstrap process.
    bootstrap_state: BootstrapState,
    /// Shutdown notify.
    shutdown: Arc<Notify>,
}

impl<C, M, P> Server<C, M, P>
where
    C: Collection,
    M: Manager,
    P: Pool,
{
    pub fn handle_request(&mut self, request: Request) {
        match request {
            Request::Get {
                key,
                local,
                respond,
            } => {
                if local {
                    self.local_get(&key, respond);
                } else {
                    self.get(key, respond);
                }
            },
            Request::Put { key, value, local } => {
                if local {
                    self.local_put(key, value);
                } else {
                    self.put(key, value);
                }
            },
            Request::ClosestContacts { key, respond } => {
                let _ = respond.send(self.manager.closest_contacts(key));
            },
            Request::Bootstrap { bootstrap_nodes } => {
                self.bootstrap(bootstrap_nodes);
            },
        }
    }

    fn bootstrap(&mut self, bootstrap_nodes: Vec<NodeIndex>) {
        if matches!(self.bootstrap_state, BootstrapState::Idle) {
            self.add_contacts(bootstrap_nodes);

            // Todo: Handle error.
            // Do self lookup.
            let (lookup_result_tx, lookup_result_rx) = oneshot::channel();
            let _ = self.pool.lookup_contact(self.us, lookup_result_tx);

            self.bootstrap_tasks
                .push(tokio::spawn(async move { lookup_result_rx.await? }));

            self.bootstrap_state = BootstrapState::Initial;
        }
    }

    fn handle_bootstrap_task_error(&mut self) {
        debug_assert!(!matches!(self.bootstrap_state, BootstrapState::Idle));

        match self.bootstrap_state {
            BootstrapState::Initial => {
                self.bootstrap_state = BootstrapState::Idle;
            },
            BootstrapState::Looking => {
                if self.bootstrap_tasks.is_empty() {
                    self.bootstrap_state = BootstrapState::Idle;
                }
            },
            BootstrapState::Idle => {
                debug_assert!(
                    false,
                    "we should not have any running bootstrap tasks during this state"
                );
            },
        }
    }

    fn handle_bootstrap_response(&mut self, contacts: Vec<NodeIndex>) {
        match self.bootstrap_state {
            BootstrapState::Initial => {
                // Add new contacts to the table.
                self.add_contacts(contacts);

                // Here we want to find the first non-empty bucket (logically to us)
                // and generate random keys for every bucket starting from
                // the aforementioned bucket.
                // We start a lookup task for each of those keys.
                let closest = self.manager.closest_contact_info(self.us);
                match closest.first() {
                    Some(node) => {
                        let index = distance::leading_zero_bits(&node.key.0, &self.us);

                        let random_keys = (index..MAX_BUCKETS)
                            .map(|index| bucket::random_key_in_bucket(index, &self.us))
                            .collect::<HashSet<_>>();

                        for next_key in random_keys {
                            let (lookup_result_tx, lookup_result_rx) = oneshot::channel();
                            let _ = self.pool.lookup_contact(next_key, lookup_result_tx);

                            self.bootstrap_tasks
                                .push(tokio::spawn(async move { lookup_result_rx.await? }));
                        }

                        self.bootstrap_state = BootstrapState::Looking;
                    },
                    None => {
                        tracing::error!("failed to find closest nodes");
                        // Nothing left to do.
                        self.bootstrap_state = BootstrapState::Idle;
                    },
                }
            },
            BootstrapState::Looking => {
                self.add_contacts(contacts);

                if self.bootstrap_tasks.is_empty() {
                    // We're done bootstrapping.
                    self.bootstrap_state = BootstrapState::Idle;
                }
            },
            BootstrapState::Idle => {
                unreachable!("invalid bootstrapping state");
            },
        }
    }

    fn add_contacts(&mut self, contacts: Vec<NodeIndex>) {
        for index in contacts {
            if let Some(key) = self.sync_query.index_to_pubkey(index) {
                let _ = self.manager.add_node(NodeInfo {
                    // Todo: Remove this field.
                    address: "0.0.0.0:0".parse().unwrap(),
                    index,
                    key,
                    last_responded: None,
                });
            }
        }
    }

    fn put(&mut self, key: TableKey, value: Bytes) {
        if let Err(e) = self.pool.store(key, value) {
            tracing::error!("`put` failed: {e:?}");
        }
    }

    fn local_put(&mut self, key: TableKey, value: Bytes) {
        self.store.insert(key, value);
    }

    fn get(&self, key: TableKey, respond: ValueRespond) {
        if self.store.contains_key(&key) {
            self.local_get(&key, respond)
        } else if let Err(e) = self.pool.lookup_value(key, respond) {
            tracing::error!("`get` failed: {e:?}");
        }
    }

    fn local_get(&self, key: &TableKey, respond: ValueRespond) {
        let entry = self.store.get(key).cloned();
        let _ = respond.send(Ok(entry));
    }

    pub async fn run(&mut self) -> Result<()> {
        loop {
            tokio::select! {
                _ = self.shutdown.notified() => {
                    break;
                }
                next = self.request_queue.recv() => {
                    let Some(request) = next else {
                        break;
                    };
                    self.handle_request(request);
                }
                next = self.event_queue.recv() => {
                    let Some(event) = next else {
                        break
                    };
                    self.manager.handle_event(event);
                }
                Some(result) = self.bootstrap_tasks.next() => {
                    match result {
                        Ok(Ok(contacts)) => {
                            self.handle_bootstrap_response(contacts);
                        }
                        _ => {
                            self.handle_bootstrap_task_error()
                        }
                    }

                }
            }
        }

        Ok(())
    }
}

pub enum Request {
    Get {
        key: TableKey,
        respond: ValueRespond,
        // Whether GET operation will be applied to local storage or DHT.
        local: bool,
    },
    Put {
        key: TableKey,
        value: Bytes,
        // Whether PUT operation will be applied to local storage or DHT.
        local: bool,
    },
    ClosestContacts {
        key: TableKey,
        respond: oneshot::Sender<Vec<NodeIndex>>,
    },
    Bootstrap {
        bootstrap_nodes: Vec<NodeIndex>,
    },
}

enum BootstrapState {
    Idle,
    Initial,
    Looking,
}
