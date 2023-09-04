use std::marker::PhantomData;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use fleek_crypto::{NodeSecretKey, SecretKey};
use lightning_interfaces::infu_collection::{c, Collection};
use lightning_interfaces::schema::broadcast::ResolvedImmutablePointerRecord;
use lightning_interfaces::types::ImmutablePointer;
use lightning_interfaces::{
    Blake3Hash,
    BroadcastInterface,
    ConfigConsumer,
    PubSub,
    ResolverInterface,
    SignerInterface,
    WithStartAndShutdown,
};
use log::error;
use rocksdb::{Options, DB};
use tokio::pin;
use tokio::sync::Notify;

use crate::config::Config;
use crate::origin_finder::OriginFinder;

const B3_TO_URI: &str = "b3_to_uri";
const URI_TO_B3: &str = "uri_to_b3";

pub struct Resolver<C: Collection> {
    pubsub: c!(C::BroadcastInterface::PubSub<ResolvedImmutablePointerRecord>),
    node_sk: NodeSecretKey,
    db: Arc<DB>,
    shutdown_notify: Arc<Notify>,
    is_running: AtomicBool,
    _collection: PhantomData<C>,
}

impl<C: Collection> ConfigConsumer for Resolver<C> {
    const KEY: &'static str = "resolver";

    type Config = Config;
}

#[async_trait]
impl<C: Collection> WithStartAndShutdown for Resolver<C> {
    /// Returns true if this system is running or not.
    fn is_running(&self) -> bool {
        self.is_running.load(Ordering::Relaxed)
    }

    /// Start the system, should not do anything if the system is already
    /// started.
    async fn start(&self) {
        if !self.is_running() {
            let mut pubsub = self.pubsub.clone();
            let db = self.db.clone();
            let shutdown_notify = self.shutdown_notify.clone();

            tokio::task::spawn(async move {
                loop {
                    // todo(dalton): revisit pinning these and using Notify over oneshot
                    let shutdown_future = shutdown_notify.notified();
                    pin!(shutdown_future);
                    tokio::select! {
                        _ = shutdown_future => break,
                        Some(msg) = pubsub.recv() => {
                            let b3_hash = msg.hash;

                            let b3_cf = db.cf_handle(B3_TO_URI).expect("No b3_to_uri column family in resolver db");
                            let uri_cf = db.cf_handle(URI_TO_B3).expect("No uri_to_b3 column family in resolver db");

                            let resolved_pointer_bytes = bincode::serialize(&msg).expect("Could not serialize pubsub message in resolver");

                            let entry = match db.get_cf(&b3_cf, b3_hash).expect("Failed to access db ") {
                                Some(bytes) => {
                                    let mut uris: Vec<ResolvedImmutablePointerRecord> = bincode::deserialize(&bytes).expect("Could not deserialize bytes in rocksdb: resolver");
                                    if !uris.iter().any(|x| x.pointer == msg.pointer){
                                        uris.push(msg);
                                    }
                                    uris

                                },
                                None => {
                                    vec![msg]
                                }
                            };
                            db.put_cf(&b3_cf, b3_hash, bincode::serialize(&entry).expect("Failed to serialize payload in resolver")).expect("Failed to insert mapping to db in resolver");
                            db.put_cf(&uri_cf, resolved_pointer_bytes, b3_hash ).expect("Failed to insert mapping to db in resolver")
                        }
                    }
                }
            });
        } else {
            error!("Can not start resolver because it already running");
        }
    }

    /// Send the shutdown signal to the system.
    async fn shutdown(&self) {
        self.shutdown_notify.notify_waiters();
        self.is_running
            .store(false, std::sync::atomic::Ordering::Relaxed)
    }
}

#[async_trait]
impl<C: Collection> ResolverInterface<C> for Resolver<C> {
    type OriginFinder = OriginFinder;

    /// Initialize and return the resolver service.
    fn init(
        config: Self::Config,
        signer: &c!(C::SignerInterface),
        pubsub: c!(C::BroadcastInterface::PubSub<ResolvedImmutablePointerRecord>),
    ) -> anyhow::Result<Self> {
        let (_, node_sk) = signer.get_sk();

        let mut db_options = Options::default();
        db_options.create_if_missing(true);

        let cf = vec![B3_TO_URI, URI_TO_B3];
        // Todo(Dalton): Configure rocksdb options
        let db = Arc::new(
            DB::open_cf(&db_options, config.store_path, cf)
                .expect("Was not able to create Resolver DB"),
        );

        Ok(Self {
            pubsub,
            node_sk,
            db,
            shutdown_notify: Arc::new(Notify::new()),
            is_running: AtomicBool::new(false),
            _collection: PhantomData,
        })
    }

    /// Publish new records into the resolver global hash table about us witnessing
    /// the given blake3 hash from resolving the following pointers.
    async fn publish(&self, hash: Blake3Hash, pointers: &[ImmutablePointer]) {
        if !pointers.is_empty() {
            // todo(dalton): actaully sign this
            let mut resolved_pointer = ResolvedImmutablePointerRecord {
                pointer: pointers[0].clone(),
                hash,
                originator: self.node_sk.to_pk(),
                signature: [0; 64].into(),
            };

            for (index, pointer) in pointers.iter().enumerate() {
                if index > 0 {
                    resolved_pointer.pointer = pointer.clone();
                }

                self.pubsub.send(&resolved_pointer).await;
            }
        }
    }

    /// Tries to find the blake3 hash of an immutable pointer by only relying on locally cached
    /// records and without performing any contact with other nodes.
    ///
    /// This can return [`None`] if no local record is found.
    async fn get_blake3_hash(
        &self,
        pointer: ImmutablePointer,
    ) -> Option<ResolvedImmutablePointerRecord> {
        let cf = self
            .db
            .cf_handle(URI_TO_B3)
            .expect("No uri_to_b3 column family in resolver db");

        let pointer_bytes = bincode::serialize(&pointer).ok()?;

        let res = self
            .db
            .get_cf(&cf, pointer_bytes)
            .expect("Failed to access db")?;

        bincode::deserialize(&res).ok()
    }

    /// Returns an origin finder that can yield origins for the provided blake3 hash.
    fn get_origin_finder(&self, _hash: Blake3Hash) -> Self::OriginFinder {
        todo!()
    }

    fn get_orgins(&self, hash: Blake3Hash) -> Option<Vec<ResolvedImmutablePointerRecord>> {
        let cf = self
            .db
            .cf_handle(B3_TO_URI)
            .expect("No b3_to_uri column family in resolver db");

        let res = self.db.get_cf(&cf, hash).expect("Failed to access db")?;

        bincode::deserialize(&res).ok()
    }
}
