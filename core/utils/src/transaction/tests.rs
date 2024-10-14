use std::collections::HashSet;
use std::time::Duration;

use anyhow::Result;
use fleek_crypto::{AccountOwnerSecretKey, NodeSecretKey, SecretKey};
use lightning_application::{Application, ApplicationConfig};
use lightning_interfaces::prelude::*;
use lightning_node::Node;
use lightning_notifier::Notifier;
use lightning_test_utils::consensus::{
    Config as MockConsensusConfig,
    MockConsensus,
    MockForwarder,
};
use lightning_test_utils::json_config::JsonConfigProvider;
use lightning_test_utils::keys::EphemeralKeystore;
use tempfile::{tempdir, TempDir};
use types::{
    ExecuteTransactionError,
    ExecuteTransactionOptions,
    ExecuteTransactionRetry,
    ExecuteTransactionWait,
    ExecutionData,
    ExecutionError,
    Genesis,
    GenesisAccount,
    GenesisNode,
    HandshakePorts,
    NodePorts,
    TransactionResponse,
    UpdateMethod,
};

use super::*;

#[tokio::test]
async fn test_execute_transaction_with_account_signer_wait_for_receipt() {
    // Build and start the node.
    let account_secret_key = AccountOwnerSecretKey::generate();
    let mut node = TestNode::<TestNodeComponents>::with_genesis_mutator(|genesis| {
        genesis.account = vec![GenesisAccount {
            public_key: account_secret_key.to_pk().into(),
            ..Default::default()
        }];
    })
    .await;
    node.start().await;

    // Build a transaction client.
    let client = node.account_transaction_client(&account_secret_key).await;

    // Check that the nonce starts at 0.
    assert_eq!(node.get_account_nonce(&account_secret_key), 0);

    // Execute a transaction and wait for it to complete.
    let (tx, receipt) = client
        .execute_transaction(
            UpdateMethod::IncrementNonce {},
            Some(ExecuteTransactionOptions {
                wait: ExecuteTransactionWait::Receipt(None),
                ..Default::default()
            }),
        )
        .await
        .unwrap()
        .as_receipt();
    assert_eq!(
        receipt.response,
        TransactionResponse::Success(ExecutionData::None)
    );
    assert!(!tx.hash().is_empty());
    assert_eq!(receipt.transaction_hash, tx.hash());

    // Check that the nonce has been incremented.
    assert_eq!(node.get_account_nonce(&account_secret_key), 1);

    // Shutdown the node.
    node.shutdown().await;
}

#[tokio::test]
async fn test_execute_transaction_with_node_signer_wait_for_receipt() {
    // Build and start the node.
    let mut node = TestNode::<TestNodeComponents>::new().await;
    node.start().await;

    // Build a transaction client.
    let signer = TransactionSigner::NodeMain(node.get_node_secret_key());
    let client = TransactionClient::<TestNodeComponents>::new(
        node.app.sync_query(),
        node.notifier.clone(),
        node.forwarder.mempool_socket(),
        signer.clone(),
    )
    .await;

    // Check that the nonce starts at 0.
    assert_eq!(signer.get_nonce(&node.app.sync_query()), 0);

    // Execute a transaction and wait for it to complete.
    let (tx, receipt) = client
        .execute_transaction(
            UpdateMethod::IncrementNonce {},
            Some(ExecuteTransactionOptions {
                wait: ExecuteTransactionWait::Receipt(None),
                ..Default::default()
            }),
        )
        .await
        .unwrap()
        .as_receipt();
    assert_eq!(
        receipt.response,
        TransactionResponse::Success(ExecutionData::None)
    );
    assert!(!tx.hash().is_empty());
    assert_eq!(receipt.transaction_hash, tx.hash());

    // Check that the nonce has been incremented.
    assert_eq!(signer.get_nonce(&node.app.sync_query()), 1);

    // Shutdown the node.
    node.shutdown().await;
}

#[tokio::test]
async fn test_execute_transaction_with_account_signer_wait_for_receipt_no_retry() {
    // Build and start the node.
    let account_secret_key = AccountOwnerSecretKey::generate();
    let mut node = TestNode::<TestNodeComponents>::with_genesis_mutator(|genesis| {
        genesis.account = vec![GenesisAccount {
            public_key: account_secret_key.to_pk().into(),
            ..Default::default()
        }];
    })
    .await;
    node.start().await;

    // Build a transaction client.
    let client = node.account_transaction_client(&account_secret_key).await;

    // Check that the nonce starts at 0.
    assert_eq!(node.get_account_nonce(&account_secret_key), 0);

    // Execute a transaction that will be reverted.
    let result = client
        .execute_transaction(
            UpdateMethod::ChangeEpoch { epoch: 101 },
            Some(ExecuteTransactionOptions {
                wait: ExecuteTransactionWait::Receipt(None),
                retry: ExecuteTransactionRetry::Never,
                ..Default::default()
            }),
        )
        .await;
    match result.unwrap_err() {
        ExecuteTransactionError::Reverted((tx, receipt)) => {
            assert_eq!(
                receipt.response,
                TransactionResponse::Revert(ExecutionError::OnlyNode)
            );
            assert!(!tx.hash().is_empty());
            assert_eq!(receipt.transaction_hash, tx.hash());
        },
        _ => panic!("unexpected error type"),
    }

    // Check that the nonce has been incremented just once.
    assert_eq!(node.get_account_nonce(&account_secret_key), 1);

    // Shutdown the node.
    node.shutdown().await;
}

#[tokio::test]
async fn test_execute_transaction_with_node_signer_wait_for_receipt_no_retry() {
    // Build and start the node.
    let mut node = TestNode::<TestNodeComponents>::new().await;
    node.start().await;

    // Build a transaction client.
    let client = node.node_transaction_client().await;

    // Check that the nonce starts at 0.
    assert_eq!(node.get_node_nonce(), 0);

    // Execute a transaction that will be reverted.
    let result = client
        .execute_transaction(
            UpdateMethod::ChangeEpoch { epoch: 101 },
            Some(ExecuteTransactionOptions {
                wait: ExecuteTransactionWait::Receipt(None),
                retry: ExecuteTransactionRetry::Never,
                ..Default::default()
            }),
        )
        .await;
    match result.unwrap_err() {
        ExecuteTransactionError::Reverted((tx, receipt)) => {
            assert_eq!(
                receipt.response,
                TransactionResponse::Revert(ExecutionError::EpochHasNotStarted)
            );
            assert!(!tx.hash().is_empty());
            assert_eq!(receipt.transaction_hash, tx.hash());
        },
        _ => panic!("unexpected error type"),
    }

    // Check that the nonce has been incremented just once.
    assert_eq!(node.get_node_nonce(), 1);

    // Shutdown the node.
    node.shutdown().await;
}

#[tokio::test]
async fn test_execute_transaction_with_account_signer_wait_for_receipt_retry_on_revert() {
    // Build and start the node.
    let account_secret_key = AccountOwnerSecretKey::generate();
    let mut node = TestNode::<TestNodeComponents>::with_genesis_mutator(|genesis| {
        genesis.account = vec![GenesisAccount {
            public_key: account_secret_key.to_pk().into(),
            ..Default::default()
        }];
    })
    .await;
    node.start().await;

    // Build a transaction client.
    let client = node.account_transaction_client(&account_secret_key).await;

    // Check that the nonce starts at 0.
    assert_eq!(node.get_account_nonce(&account_secret_key), 0);

    // Execute a transaction that will be reverted.
    let result = client
        .execute_transaction(
            UpdateMethod::ChangeEpoch { epoch: 101 },
            Some(ExecuteTransactionOptions {
                wait: ExecuteTransactionWait::Receipt(None),
                // `None` means to use the default max retries.
                retry: ExecuteTransactionRetry::Always(None),
                ..Default::default()
            }),
        )
        .await;
    match result.unwrap_err() {
        ExecuteTransactionError::Reverted((tx, receipt)) => {
            assert_eq!(
                receipt.response,
                TransactionResponse::Revert(ExecutionError::OnlyNode)
            );
            assert!(!tx.hash().is_empty());
            assert_eq!(receipt.transaction_hash, tx.hash());
        },
        _ => panic!("unexpected error type"),
    }

    // Check that the nonce has been incremented 4 times, for the initial attempt and each retry.
    assert_eq!(node.get_account_nonce(&account_secret_key), 4);

    // Shutdown the node.
    node.shutdown().await;
}

#[tokio::test]
async fn test_execute_transaction_with_node_signer_wait_for_receipt_retry_on_revert() {
    // Build and start the node.
    let mut node = TestNode::<TestNodeComponents>::new().await;
    node.start().await;

    // Build a transaction client.
    let client = node.node_transaction_client().await;

    // Check that the nonce starts at 0.
    assert_eq!(node.get_node_nonce(), 0);

    // Execute a transaction that will be reverted.
    let result = client
        .execute_transaction(
            UpdateMethod::ChangeEpoch { epoch: 101 },
            Some(ExecuteTransactionOptions {
                wait: ExecuteTransactionWait::Receipt(None),
                // `None` means to use the default max retries.
                retry: ExecuteTransactionRetry::Always(None),
                ..Default::default()
            }),
        )
        .await;
    match result.unwrap_err() {
        ExecuteTransactionError::Reverted((tx, receipt)) => {
            assert_eq!(
                receipt.response,
                TransactionResponse::Revert(ExecutionError::EpochHasNotStarted)
            );
            assert!(!tx.hash().is_empty());
            assert_eq!(receipt.transaction_hash, tx.hash());
        },
        _ => panic!("unexpected error type"),
    }

    // Check that the nonce has been incremented 4 times, for the initial attempt and each retry.
    assert_eq!(node.get_node_nonce(), 4);

    // Shutdown the node.
    node.shutdown().await;
}

partial_node_components!(TestNodeComponents {
    ConfigProviderInterface = JsonConfigProvider;
    ApplicationInterface = Application<Self>;
    NotifierInterface = Notifier<Self>;
    KeystoreInterface = EphemeralKeystore<Self>;
    ConsensusInterface = MockConsensus<Self>;
    ForwarderInterface = MockForwarder<Self>;
});

struct TestNode<C: NodeComponents> {
    inner: Node<C>,
    _temp_dir: TempDir,

    app: fdi::Ref<C::ApplicationInterface>,
    notifier: fdi::Ref<C::NotifierInterface>,
    forwarder: fdi::Ref<C::ForwarderInterface>,
}

impl<C: NodeComponents> TestNode<C> {
    pub async fn new() -> Self {
        Self::with_genesis_mutator(|_| {}).await
    }

    pub async fn with_genesis_mutator(mutator: impl FnOnce(&mut Genesis)) -> Self {
        let temp_dir = tempdir().unwrap();
        let inner = Self::build_inner(&temp_dir, mutator).await.unwrap();

        let app = inner.provider.get::<C::ApplicationInterface>();
        let notifier = inner.provider.get::<C::NotifierInterface>();
        let forwarder = inner.provider.get::<C::ForwarderInterface>();

        Self {
            inner,
            _temp_dir: temp_dir,

            app,
            notifier,
            forwarder,
        }
    }

    pub async fn start(&mut self) {
        self.inner.start().await;
    }

    pub async fn shutdown(&mut self) {
        self.inner.shutdown().await;
    }

    pub fn get_node_secret_key(&self) -> NodeSecretKey {
        self.inner
            .provider
            .get::<C::KeystoreInterface>()
            .get_ed25519_sk()
    }

    pub async fn node_transaction_client(&self) -> TransactionClient<C> {
        TransactionClient::<C>::new(
            self.app.sync_query(),
            self.notifier.clone(),
            self.forwarder.mempool_socket(),
            TransactionSigner::NodeMain(self.get_node_secret_key()),
        )
        .await
    }

    pub async fn account_transaction_client(
        &self,
        account_secret_key: &AccountOwnerSecretKey,
    ) -> TransactionClient<C> {
        TransactionClient::<C>::new(
            self.app.sync_query(),
            self.notifier.clone(),
            self.forwarder.mempool_socket(),
            TransactionSigner::AccountOwner(account_secret_key.clone()),
        )
        .await
    }

    pub fn get_node_nonce(&self) -> u64 {
        TransactionSigner::NodeMain(self.get_node_secret_key()).get_nonce(&self.app.sync_query())
    }

    pub fn get_account_nonce(&self, account_secret_key: &AccountOwnerSecretKey) -> u64 {
        TransactionSigner::AccountOwner(account_secret_key.clone())
            .get_nonce(&self.app.sync_query())
    }

    async fn build_inner(
        temp_dir: &TempDir,
        genesis_mutator: impl FnOnce(&mut Genesis),
    ) -> Result<Node<C>> {
        let keystore = EphemeralKeystore::<C>::default();
        let node_secret_key = keystore.get_ed25519_sk();
        let mut genesis = Genesis {
            node_info: vec![GenesisNode {
                owner: AccountOwnerSecretKey::generate().to_pk().into(),
                primary_public_key: node_secret_key.to_pk(),
                primary_domain: "127.0.0.1".parse().unwrap(),
                consensus_public_key: keystore.get_bls_pk(),
                worker_domain: "127.0.0.1".parse().unwrap(),
                worker_public_key: node_secret_key.to_pk(),
                ports: NodePorts {
                    primary: 0,
                    worker: 0,
                    mempool: 0,
                    rpc: 0,
                    pool: 0,
                    pinger: 0,
                    handshake: HandshakePorts {
                        http: 0,
                        webrtc: 0,
                        webtransport: 0,
                    },
                },
                stake: Default::default(),
                reputation: None,
                current_epoch_served: None,
                genesis_committee: true,
            }],
            ..Default::default()
        };
        genesis_mutator(&mut genesis);
        let genesis_path = genesis
            .write_to_dir(temp_dir.path().to_path_buf().try_into().unwrap())
            .unwrap();
        Node::<C>::init_with_provider(
            fdi::Provider::default().with(keystore).with(
                JsonConfigProvider::default()
                    .with::<Application<C>>(ApplicationConfig::test(genesis_path))
                    .with::<MockConsensus<C>>(MockConsensusConfig {
                        min_ordering_time: 0,
                        max_ordering_time: 0,
                        probability_txn_lost: 0.0,
                        transactions_to_lose: HashSet::new(),
                        new_block_interval: Duration::from_secs(0),
                    }),
            ),
        )
        .map_err(anyhow::Error::from)
    }
}
