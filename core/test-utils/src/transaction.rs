use ethers::types::{Transaction as EthersTransaction, U256};
use ethers::utils::rlp;
use fleek_crypto::{EthAddress, NodeSecretKey, SecretKey, TransactionSender, TransactionSignature};
use lightning_interfaces::types::{
    Block,
    BlockExecutionResponse,
    ExecutionData,
    IndexRequest,
    TransactionDestination,
    TransactionReceipt,
    TransactionRequest,
    TransactionResponse,
    UpdateMethod,
    UpdatePayload,
    UpdateRequest,
};
use lightning_interfaces::ToDigest;

pub fn get_update_transactions(num_txns: usize) -> Vec<UpdateRequest> {
    (0..num_txns)
        .map(|i| {
            let secret_key = NodeSecretKey::generate();
            let public_key = secret_key.to_pk();
            let sender = TransactionSender::NodeMain(public_key);
            let method = UpdateMethod::ChangeEpoch { epoch: i as u64 };
            let payload = UpdatePayload {
                sender,
                nonce: 0,
                method,
                chain_id: 1337,
            };
            let digest = payload.to_digest();
            UpdateRequest {
                signature: TransactionSignature::NodeMain(secret_key.sign(&digest)),
                payload,
            }
        })
        .collect()
}

pub fn get_eth_transactions(num_txns: usize) -> Vec<EthersTransaction> {
    (0..num_txns)
        .map(|i| {
            let tx = EthersTransaction {
                nonce: U256::from(i),
                ..Default::default()
            };
            let bytes = tx.rlp().to_vec();
            rlp::decode::<EthersTransaction>(&bytes).unwrap()
        })
        .collect()
}

pub fn get_index_request(block_index: u8, parent_hash: [u8; 32]) -> IndexRequest {
    let digest = [block_index; 32];
    let update_txns = get_update_transactions(5);
    let eth_txns = get_eth_transactions(5);
    let to = TransactionDestination::Ethereum(EthAddress([0; 20]));

    let mut txn_receipts = Vec::new();
    for (i, tx) in update_txns.iter().enumerate() {
        txn_receipts.push(TransactionReceipt {
            block_hash: digest,
            block_number: block_index as u64,
            transaction_index: i as u64,
            transaction_hash: tx.payload.to_digest(), // dummy hash
            from: tx.payload.sender,
            to: to.clone(),
            response: TransactionResponse::Success(ExecutionData::None),
            event: None,
        });
    }

    for (i, tx) in eth_txns.iter().enumerate() {
        txn_receipts.push(TransactionReceipt {
            block_hash: digest,
            block_number: block_index as u64,
            transaction_index: i as u64,
            transaction_hash: tx.hash.0, // dummy hash
            from: TransactionSender::AccountOwner(EthAddress(tx.from.0)),
            to: to.clone(),
            response: TransactionResponse::Success(ExecutionData::None),
            event: None,
        });
    }

    let txn_receipts: Vec<TransactionReceipt> = update_txns
        .iter()
        .enumerate()
        .map(|(i, tx)| {
            TransactionReceipt {
                block_hash: digest,
                block_number: block_index as u64,
                transaction_index: i as u64,
                transaction_hash: tx.payload.to_digest(), // dummy hash
                from: tx.payload.sender,
                to: to.clone(),
                response: TransactionResponse::Success(ExecutionData::None),
                event: None,
            }
        })
        .collect();

    let mut transactions = Vec::new();
    for tx in update_txns {
        transactions.push(TransactionRequest::UpdateRequest(tx));
    }
    for tx in eth_txns {
        transactions.push(TransactionRequest::EthereumRequest(tx.into()));
    }
    let block = Block {
        transactions,
        digest,
    };

    let receipt = BlockExecutionResponse {
        block_number: block_index as u64,
        block_hash: digest,
        parent_hash,
        change_epoch: false,
        node_registry_delta: vec![],
        txn_receipts,
    };

    IndexRequest { block, receipt }
}
