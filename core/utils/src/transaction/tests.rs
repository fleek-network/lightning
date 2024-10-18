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
use lightning_test_utils::e2e::try_init_tracing;
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
    TransactionRequest,
    TransactionResponse,
    UpdateMethod,
};

use super::*;
use crate::poll::{poll_until, PollUntilError};

#[tokio::test]
async fn test_execute_transaction_with_account_signer_wait_for_receipt() {
    // Build and start the node.
    let account_secret_key = AccountOwnerSecretKey::generate();
    let account_secret_key_clone = account_secret_key.clone();
    let mut node = TestNodeBuilder::new()
        .with_genesis_mutator(move |genesis| {
            genesis.account = vec![GenesisAccount {
                public_key: account_secret_key_clone.to_pk().into(),
                ..Default::default()
            }];
        })
        .build::<TestNodeComponents>()
        .await
        .unwrap();
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
    assert_eq!(node.get_account_nonce(&account_secret_key), 1);

    // Shutdown the node.
    node.shutdown().await;
}

#[tokio::test]
async fn test_execute_transaction_with_node_signer_wait_for_receipt() {
    // Build and start the node.
    let mut node = TestNodeBuilder::new()
        .build::<TestNodeComponents>()
        .await
        .unwrap();
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
    assert_eq!(signer.get_nonce(&node.app.sync_query()), 1);

    // Shutdown the node.
    node.shutdown().await;
}

#[tokio::test]
async fn test_execute_transaction_with_account_signer_wait_for_receipt_no_retry() {
    // Build and start the node.
    let account_secret_key = AccountOwnerSecretKey::generate();
    let account_secret_key_clone = account_secret_key.clone();
    let mut node = TestNodeBuilder::new()
        .with_genesis_mutator(move |genesis| {
            genesis.account = vec![GenesisAccount {
                public_key: account_secret_key_clone.to_pk().into(),
                ..Default::default()
            }];
        })
        .build::<TestNodeComponents>()
        .await
        .unwrap();
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
                TransactionResponse::Revert(ExecutionError::OnlyNode)
            );
            assert!(!tx.hash().is_empty());
            assert_eq!(receipt.transaction_hash, tx.hash());
            assert_eq!(attempts, 1);
        },
        e => panic!("unexpected error type: {e:?}"),
    }

    // Check that the nonce has been incremented just once.
    assert_eq!(node.get_account_nonce(&account_secret_key), 1);

    // Shutdown the node.
    node.shutdown().await;
}

#[tokio::test]
async fn test_execute_transaction_with_node_signer_wait_for_receipt_no_retry() {
    // Build and start the node.
    let mut node = TestNodeBuilder::new()
        .build::<TestNodeComponents>()
        .await
        .unwrap();
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
    assert_eq!(node.get_node_nonce(), 1);

    // Shutdown the node.
    node.shutdown().await;
}

#[tokio::test]
async fn test_execute_transaction_with_account_signer_wait_for_receipt_retry_on_revert() {
    // Build and start the node.
    let account_secret_key = AccountOwnerSecretKey::generate();
    let account_secret_key_clone = account_secret_key.clone();
    let mut node = TestNodeBuilder::new()
        .with_genesis_mutator(move |genesis| {
            genesis.account = vec![GenesisAccount {
                public_key: account_secret_key_clone.to_pk().into(),
                ..Default::default()
            }];
        })
        .build::<TestNodeComponents>()
        .await
        .unwrap();
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
                TransactionResponse::Revert(ExecutionError::OnlyNode)
            );
            assert!(!tx.hash().is_empty());
            assert_eq!(receipt.transaction_hash, tx.hash());
            assert_eq!(attempts, 4);
        },
        e => panic!("unexpected error type: {e:?}"),
    }

    // Check that the nonce has been incremented 4 times, for the initial attempt and each retry.
    assert_eq!(node.get_account_nonce(&account_secret_key), 4);

    // Shutdown the node.
    node.shutdown().await;
}

#[tokio::test]
async fn test_execute_transaction_with_node_signer_wait_for_receipt_retry_on_revert() {
    // Build and start the node.
    let mut node = TestNodeBuilder::new()
        .build::<TestNodeComponents>()
        .await
        .unwrap();
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
    assert_eq!(node.get_node_nonce(), 4);

    // Shutdown the node.
    node.shutdown().await;
}

#[tokio::test]
async fn test_execute_transaction_with_node_signer_wait_for_receipt_retry_on_timeout() {
    // Build and start the node.
    let mut node = TestNodeBuilder::new()
        .with_mock_consensus_config(MockConsensusConfig {
            probability_txn_lost: 1.0,
            ..Default::default()
        })
        .build::<TestNodeComponents>()
        .await
        .unwrap();
    node.start().await;

    // Build a transaction client.
    let client = node.node_transaction_client().await;

    // Check that the nonce starts at 0.
    assert_eq!(node.get_node_nonce(), 0);

    // Execute a transaction that will be lost.
    let result = client
        .execute_transaction(
            UpdateMethod::IncrementNonce {},
            Some(ExecuteTransactionOptions {
                wait: ExecuteTransactionWait::Receipt,
                retry: ExecuteTransactionRetry::Always(None),
                timeout: Some(Duration::from_millis(200)),
            }),
        )
        .await;
    match result.unwrap_err() {
        ExecuteTransactionError::Timeout((_, _, attempts)) => {
            // Check that there were 4 attempts; 1 initial attempt and 3 retries.
            assert_eq!(attempts, 4);
        },
        e => panic!("unexpected error type: {e:?}"),
    }

    // Check that the nonce has not been incremented since no transaction was included.
    assert_eq!(node.get_node_nonce(), 0);

    // Shutdown the node.
    node.shutdown().await;
}

#[tokio::test]
async fn test_execute_transaction_wait_for_receipt_retry_on_timeout_avoids_resubmission() {
    // Build and start the node.
    let mut node = TestNodeBuilder::new()
        .with_mock_consensus_config(MockConsensusConfig {
            min_ordering_time: 0,
            max_ordering_time: 0,
            probability_txn_lost: 0.0,
            // Lose the first transaction.
            transactions_to_lose: HashSet::from_iter(vec![1]),
            new_block_interval: Duration::from_secs(0),
            block_buffering_interval: Duration::from_secs(0),
        })
        .with_genesis_mutator(move |genesis| {
            genesis.chain_id = 1337;
        })
        .build::<TestNodeComponents>()
        .await
        .unwrap();
    node.start().await;

    // Build a transaction client.
    let client = node.node_transaction_client().await;

    // Check that the nonce starts at 0.
    assert_eq!(node.get_node_nonce(), 0);

    // Execute a transaction that will be initially lost.
    let (tx, receipt) = client
        .execute_transaction(
            UpdateMethod::UpdateContentRegistry {
                updates: Default::default(),
            },
            Some(ExecuteTransactionOptions {
                wait: ExecuteTransactionWait::Receipt,
                retry: ExecuteTransactionRetry::Always(None),
                timeout: Some(Duration::from_millis(200)),
            }),
        )
        .await
        .unwrap()
        .as_receipt();

    // Check that the transaction was executed successfully with the expected nonce.
    assert_eq!(
        receipt.response,
        TransactionResponse::Success(ExecutionData::None)
    );
    let nonce = match &tx {
        TransactionRequest::UpdateRequest(tx) => tx.payload.nonce,
        TransactionRequest::EthereumRequest(tx) => tx.tx.nonce.as_u64(),
    };
    assert_eq!(nonce, 2);
    assert!(!tx.hash().is_empty());
    assert_eq!(receipt.transaction_hash, tx.hash());

    // Check that the nonce has been incremented twice.
    assert_eq!(node.get_node_nonce(), 2);

    // Shutdown the node.
    node.shutdown().await;
}

#[tokio::test]
async fn test_execute_transaction_no_wait_for_receipt_retry_on_timeout_avoids_resubmission() {
    // Build and start the node.
    let mut node = TestNodeBuilder::new()
        .with_mock_consensus_config(MockConsensusConfig {
            min_ordering_time: 0,
            max_ordering_time: 0,
            probability_txn_lost: 0.0,
            // Lose the first transaction.
            transactions_to_lose: HashSet::from_iter(vec![1]),
            new_block_interval: Duration::from_secs(0),
            block_buffering_interval: Duration::from_secs(0),
        })
        .with_genesis_mutator(move |genesis| {
            genesis.chain_id = 1337;
        })
        .build::<TestNodeComponents>()
        .await
        .unwrap();
    node.start().await;

    // Build a transaction client.
    let client = node.node_transaction_client().await;

    // Check that the nonce starts at 0.
    assert_eq!(node.get_node_nonce(), 0);

    // Execute a transaction that will be initially lost.
    client
        .execute_transaction(
            UpdateMethod::UpdateContentRegistry {
                updates: Default::default(),
            },
            Some(ExecuteTransactionOptions {
                wait: ExecuteTransactionWait::None,
                retry: ExecuteTransactionRetry::Always(None),
                timeout: Some(Duration::from_millis(200)),
            }),
        )
        .await
        .unwrap();

    // Check that the nonce is eventually incremented twice.
    poll_until(
        || async {
            (node.get_node_nonce() == 2)
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
});

struct TestNode<C: NodeComponents> {
    inner: Node<C>,
    _temp_dir: TempDir,

    app: fdi::Ref<C::ApplicationInterface>,
    notifier: fdi::Ref<C::NotifierInterface>,
    forwarder: fdi::Ref<C::ForwarderInterface>,
}

impl<C: NodeComponents> TestNode<C> {
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
}

type GenesisMutator = Box<dyn FnOnce(&mut Genesis)>;

struct TestNodeBuilder {
    genesis_mutator: Option<GenesisMutator>,
    mock_consensus_config: Option<MockConsensusConfig>,
}

impl TestNodeBuilder {
    pub fn new() -> Self {
        Self {
            genesis_mutator: None,
            mock_consensus_config: None,
        }
    }

    pub fn with_genesis_mutator(mut self, mutator: impl FnOnce(&mut Genesis) + 'static) -> Self {
        self.genesis_mutator = Some(Box::new(mutator));
        self
    }

    pub fn with_mock_consensus_config(mut self, config: MockConsensusConfig) -> Self {
        self.mock_consensus_config = Some(config);
        self
    }

    pub async fn build<C: NodeComponents>(self) -> Result<TestNode<C>> {
        let _ = try_init_tracing();
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
        if let Some(mutator) = self.genesis_mutator {
            mutator(&mut genesis);
        }
        let temp_dir = tempdir().unwrap();
        let genesis_path = genesis
            .write_to_dir(temp_dir.path().to_path_buf().try_into().unwrap())
            .unwrap();
        let node = Node::<C>::init_with_provider(
            fdi::Provider::default().with(keystore).with(
                JsonConfigProvider::default()
                    .with::<Application<C>>(ApplicationConfig::test(genesis_path))
                    .with::<MockConsensus<C>>(self.mock_consensus_config.unwrap_or(
                        MockConsensusConfig {
                            min_ordering_time: 0,
                            max_ordering_time: 0,
                            probability_txn_lost: 0.0,
                            transactions_to_lose: HashSet::new(),
                            new_block_interval: Duration::from_secs(0),
                            block_buffering_interval: Duration::from_secs(0),
                        },
                    )),
            ),
        )
        .map_err(anyhow::Error::from)?;
        Ok(TestNode {
            app: node.provider.get::<C::ApplicationInterface>(),
            notifier: node.provider.get::<C::NotifierInterface>(),
            forwarder: node.provider.get::<C::ForwarderInterface>(),

            inner: node,
            _temp_dir: temp_dir,
        })
    }
}
