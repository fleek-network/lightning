use std::collections::HashSet;
use std::time::Duration;

use anyhow::Result;
use fleek_crypto::{AccountOwnerSecretKey, NodeSecretKey, SecretKey};
use lightning_application::{Application, ApplicationConfig};
use lightning_interfaces::prelude::*;
use lightning_node::Node;
use lightning_notifier::Notifier;
use lightning_test_utils::consensus::{MockConsensus, MockConsensusConfig, MockForwarder};
use lightning_test_utils::json_config::JsonConfigProvider;
use lightning_test_utils::keys::EphemeralKeystore;
use lightning_utils::poll::{poll_until, PollUntilError};
use lightning_utils::transaction::TransactionSigner;
use tempfile::{tempdir, TempDir};
use types::{
    ExecuteTransactionError,
    ExecuteTransactionOptions,
    ExecuteTransactionRequest,
    ExecuteTransactionResponse,
    ExecuteTransactionRetry,
    ExecuteTransactionWait,
    ExecutionData,
    ExecutionError,
    Genesis,
    GenesisNode,
    HandshakePorts,
    NodePorts,
    TransactionResponse,
    UpdateMethod,
};

use super::*;

#[tokio::test]
async fn test_execute_transaction_and_wait_for_receipt_success() {
    // Build and start the node.
    let mut node = TestNode::<TestNodeComponents>::new().await;
    node.start().await;

    // Check that the nonce starts at 0.
    assert_eq!(node.get_nonce(), 0);

    // Execute a transaction and wait for it to complete.
    let (tx, receipt) = node
        .execute_transaction(
            UpdateMethod::IncrementNonce {},
            Some(ExecuteTransactionOptions {
                wait: ExecuteTransactionWait::Receipt,
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
    assert_eq!(node.get_nonce(), 1);

    // Execute a few more transactions.
    for _ in 0..10 {
        let (tx, receipt) = node
            .execute_transaction(
                UpdateMethod::IncrementNonce {},
                Some(ExecuteTransactionOptions {
                    wait: ExecuteTransactionWait::Receipt,
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
        assert_eq!(receipt.transaction_hash, tx.hash());
    }

    // Check that the nonce has been incremented.
    assert_eq!(node.get_nonce(), 11);

    // Shutdown the node.
    node.shutdown().await;
}

#[tokio::test]
async fn test_execute_transaction_and_no_wait_for_receipt_success() {
    // Build and start the node.
    let mut node = TestNode::<TestNodeComponents>::new().await;
    node.start().await;

    // Check that the nonce starts at 0.
    assert_eq!(node.get_nonce(), 0);

    // Execute a transaction and wait for it to complete.
    node.execute_transaction(
        UpdateMethod::IncrementNonce {},
        Some(ExecuteTransactionOptions {
            wait: ExecuteTransactionWait::None,
            ..Default::default()
        }),
    )
    .await
    .unwrap()
    .as_none();

    // Wait for the nonce to be incremented.
    poll_until(
        || async {
            (node.get_nonce() == 1)
                .then_some(())
                .ok_or(PollUntilError::ConditionNotSatisfied)
        },
        Duration::from_secs(5),
        Duration::from_millis(100),
    )
    .await
    .unwrap();

    // Execute a few more transactions.
    for _ in 0..10 {
        node.execute_transaction(
            UpdateMethod::IncrementNonce {},
            Some(ExecuteTransactionOptions {
                wait: ExecuteTransactionWait::None,
                ..Default::default()
            }),
        )
        .await
        .unwrap()
        .as_none();
    }

    // Wait for the nonce to be incremented for every transaction.
    poll_until(
        || async {
            (node.get_nonce() == 11)
                .then_some(())
                .ok_or(PollUntilError::ConditionNotSatisfied)
        },
        Duration::from_secs(5),
        Duration::from_millis(100),
    )
    .await
    .unwrap();

    // Shutdown the node.
    node.shutdown().await;
}

#[tokio::test]
async fn test_execute_transaction_and_wait_for_receipt_without_retry_on_revert() {
    // Build and start the node.
    let mut node = TestNode::<TestNodeComponents>::new().await;
    node.start().await;

    // Check that the nonce starts at 0.
    assert_eq!(node.get_nonce(), 0);

    // Execute a transaction that will be reverted.
    let result = node
        .execute_transaction(
            UpdateMethod::ChangeEpoch { epoch: 101 },
            Some(ExecuteTransactionOptions {
                wait: ExecuteTransactionWait::Receipt,
                retry: ExecuteTransactionRetry::Never,
                timeout: None,
            }),
        )
        .await;
    match result.unwrap_err() {
        ExecuteTransactionError::Reverted((tx, receipt, attempts)) => {
            assert_eq!(
                receipt.response,
                TransactionResponse::Revert(ExecutionError::EpochHasNotStarted)
            );
            assert!(!tx.hash().is_empty());
            assert_eq!(receipt.transaction_hash, tx.hash());
            assert_eq!(attempts, 1);
        },
        e => panic!("unexpected error type: {e:?}"),
    }

    // Check that the nonce has been incremented just once.
    assert_eq!(node.get_nonce(), 1);

    // Shutdown the node.
    node.shutdown().await;
}

#[tokio::test]
async fn test_execute_transaction_and_no_wait_for_receipt_without_retry_on_revert() {
    // Build and start the node.
    let mut node = TestNode::<TestNodeComponents>::new().await;
    node.start().await;

    // Check that the nonce starts at 0.
    assert_eq!(node.get_nonce(), 0);

    // Execute a transaction that will be reverted.
    let resp = node
        .execute_transaction(
            UpdateMethod::ChangeEpoch { epoch: 101 },
            Some(ExecuteTransactionOptions {
                wait: ExecuteTransactionWait::None,
                retry: ExecuteTransactionRetry::Never,
                timeout: None,
            }),
        )
        .await
        .unwrap();
    assert_eq!(resp, ExecuteTransactionResponse::None);

    // Check that the nonce has been incremented just once.
    poll_until(
        || async {
            (node.get_nonce() == 1)
                .then_some(())
                .ok_or(PollUntilError::ConditionNotSatisfied)
        },
        Duration::from_secs(5),
        Duration::from_millis(100),
    )
    .await
    .unwrap();

    // Shutdown the node.
    node.shutdown().await;
}

#[tokio::test]
async fn test_execute_transaction_and_wait_for_receipt_retry_on_revert() {
    // Build and start the node.
    let mut node = TestNode::<TestNodeComponents>::new().await;
    node.start().await;

    // Check that the nonce starts at 0.
    assert_eq!(node.get_nonce(), 0);

    // Execute a transaction that will be reverted.
    let result = node
        .execute_transaction(
            UpdateMethod::ChangeEpoch { epoch: 101 },
            Some(ExecuteTransactionOptions {
                wait: ExecuteTransactionWait::Receipt,
                // `None` means to use the default max retries.
                retry: ExecuteTransactionRetry::Always(None),
                timeout: None,
            }),
        )
        .await;
    match result.unwrap_err() {
        ExecuteTransactionError::Reverted((tx, receipt, attempts)) => {
            assert_eq!(
                receipt.response,
                TransactionResponse::Revert(ExecutionError::EpochHasNotStarted)
            );
            assert!(!tx.hash().is_empty());
            assert_eq!(receipt.transaction_hash, tx.hash());
            assert_eq!(attempts, 4);
        },
        e => panic!("unexpected error type: {e:?}"),
    }

    // Check that the nonce has been incremented 4 times, for the initial attempt and each retry.
    assert_eq!(node.get_nonce(), 4);

    // Shutdown the node.
    node.shutdown().await;
}

#[tokio::test]
async fn test_execute_transaction_and_no_wait_for_receipt_retry_on_revert() {
    // Build and start the node.
    let mut node = TestNode::<TestNodeComponents>::new().await;
    node.start().await;

    // Check that the nonce starts at 0.
    assert_eq!(node.get_nonce(), 0);

    // Execute a transaction that will be reverted.
    let resp = node
        .execute_transaction(
            UpdateMethod::ChangeEpoch { epoch: 101 },
            Some(ExecuteTransactionOptions {
                wait: ExecuteTransactionWait::None,
                // `None` means to use the default max retries.
                retry: ExecuteTransactionRetry::Always(None),
                timeout: None,
            }),
        )
        .await
        .unwrap();
    assert_eq!(resp, ExecuteTransactionResponse::None);

    // Check that the nonce has been incremented 4 times, for the initial attempt and each retry.
    poll_until(
        || async {
            (node.get_nonce() == 4)
                .then_some(())
                .ok_or(PollUntilError::ConditionNotSatisfied)
        },
        Duration::from_secs(5),
        Duration::from_millis(100),
    )
    .await
    .unwrap();

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
    SignerInterface = Signer<Self>;
});

struct TestNode<C: NodeComponents> {
    inner: Node<C>,
    _temp_dir: TempDir,

    app: fdi::Ref<C::ApplicationInterface>,
    signer: fdi::Ref<C::SignerInterface>,
}

impl<C: NodeComponents> TestNode<C> {
    pub async fn new() -> Self {
        Self::with_genesis_mutator(|_| {}).await
    }

    pub async fn with_genesis_mutator(mutator: impl FnOnce(&mut Genesis)) -> Self {
        let temp_dir = tempdir().unwrap();
        let inner = Self::build_inner(&temp_dir, mutator).await.unwrap();

        let app = inner.provider.get::<C::ApplicationInterface>();
        let signer = inner.provider.get::<C::SignerInterface>();

        Self {
            inner,
            _temp_dir: temp_dir,

            app,
            signer,
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

    pub fn get_nonce(&self) -> u64 {
        TransactionSigner::NodeMain(self.get_node_secret_key()).get_nonce(&self.app.sync_query())
    }

    pub async fn execute_transaction(
        &self,
        method: UpdateMethod,
        options: Option<ExecuteTransactionOptions>,
    ) -> Result<ExecuteTransactionResponse, ExecuteTransactionError> {
        let resp = self
            .signer
            .get_socket()
            .run(ExecuteTransactionRequest { method, options })
            .await??;

        Ok(resp)
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
                        block_buffering_interval: Duration::from_secs(0),
                    }),
            ),
        )
        .map_err(anyhow::Error::from)
    }
}
