use std::sync::Arc;
use std::time::Duration;

use ethers::middleware::SignerMiddleware;
use ethers::providers::Middleware;
use ethers::signers::{LocalWallet, Signer};
use fleek_crypto::{AccountOwnerSecretKey, EthAddress, SecretKey};
use lightning_e2e::swarm::Swarm;
use lightning_interfaces::_SyncQueryRunnerInterface;
use lightning_node_bindings::FullNodeComponents;
use lightning_test_utils::logging;
use lightning_utils::eth::FleekContract;
use lightning_utils::poll::poll_until;
use tempfile::tempdir;

#[tokio::test]
async fn e2e_eth_client_approve_revoke() {
    logging::setup(None);

    let temp_dir = tempdir().unwrap();
    let chain_id = 1337;
    let mut swarm = Swarm::<FullNodeComponents>::builder()
        .with_directory(temp_dir.path().to_path_buf().try_into().unwrap())
        .with_min_port(20000)
        .with_num_nodes(4)
        .with_commit_phase_time(3000)
        .with_reveal_phase_time(3000)
        .with_chain_id(chain_id)
        .persistence(true)
        .with_archiver()
        .build::<FullNodeComponents>();
    swarm.launch().await.unwrap();

    // Wait for RPC to be ready.
    swarm.wait_for_rpc_ready().await;

    // give forwarder some time to connect
    // TODO: this shouldn't be needed, but something is weird with the start of each epoch
    tokio::time::sleep(Duration::from_secs(4)).await;

    let rpc_addr = swarm.get_rpc_addresses().into_iter().next().unwrap().1;

    let provider = ethers::providers::Provider::<ethers::providers::Http>::try_from(rpc_addr)
        .expect("to connect to node rpc");

    let secret = AccountOwnerSecretKey::generate();
    let account_pk: EthAddress = secret.to_pk().into();
    let wallet = LocalWallet::from_bytes(&secret.into_bytes()).unwrap();
    let client = Arc::new(SignerMiddleware::new(provider.clone(), wallet.clone()));

    let contract = FleekContract::new(
        "0x0606060606060606060606060606060606060606"
            .parse::<ethers::types::Address>()
            .unwrap(),
        client.clone(),
    );

    // create a client key
    let client_key = fleek_crypto::ClientSecretKey::generate();
    let client_pk = client_key.to_pk();

    // Create a transaction for the client key approval, sign, and send it to the node
    let mut tx = contract.approve_client_key(client_pk.0.into()).tx;
    tx.set_chain_id(chain_id);
    tx.set_nonce(1);
    let sig = client
        .sign_transaction(&tx, wallet.address())
        .await
        .unwrap();
    let res = provider
        .send_raw_transaction(tx.rlp_signed(&sig))
        .await
        .unwrap();

    // poll for a transaction receipt
    // TODO: correctly provide eth_getTransactionByHash so we can just await the pending
    // transaction with ethers contract abstraction
    let _receipt = poll_until(
        || async {
            provider
                .get_transaction_receipt(res.0)
                .await
                .ok()
                .flatten()
                .ok_or(lightning_utils::poll::PollUntilError::ConditionNotSatisfied)
        },
        Duration::from_secs(10),
        Duration::from_millis(500),
    )
    .await
    .unwrap();
    assert_eq!(_receipt.status, Some(1.into()));

    // grab a query runner and ensure the client key is set to the desired key
    let (_, query) = swarm.get_query_runners().into_iter().next().unwrap();
    let app_client_key = query
        .get_account_info(&account_pk, |x| x.client_key)
        .unwrap();
    assert_eq!(app_client_key, Some(client_pk));

    // Create a transaction for the client key revokation, sign, and send it to the node
    let mut tx = contract.revoke_client_key().tx;
    tx.set_chain_id(chain_id);
    tx.set_nonce(2);
    let sig = client
        .sign_transaction(&tx, wallet.address())
        .await
        .unwrap();
    let res = provider
        .send_raw_transaction(tx.rlp_signed(&sig))
        .await
        .unwrap();

    // poll for a transaction receipt
    // TODO: correctly provide eth_getTransactionByHash so we can just await the pending
    // transaction with ethers contract abstraction
    let _receipt = poll_until(
        || async {
            provider
                .get_transaction_receipt(res.0)
                .await
                .ok()
                .flatten()
                .ok_or(lightning_utils::poll::PollUntilError::ConditionNotSatisfied)
        },
        Duration::from_secs(10),
        Duration::from_millis(500),
    )
    .await
    .unwrap();
    assert_eq!(_receipt.status, Some(1.into()));

    // ensure the client key has been cleared
    let app_client_key = query
        .get_account_info(&account_pk, |x| x.client_key)
        .unwrap();
    assert_eq!(app_client_key, None);

    swarm.shutdown().await;
}
