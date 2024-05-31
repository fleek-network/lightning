use ethers::types::{Transaction as EthersTransaction, U256};
use ethers::utils::rlp;
use fleek_crypto::{NodeSecretKey, SecretKey, TransactionSender, TransactionSignature};
use lightning_interfaces::types::{UpdateMethod, UpdatePayload, UpdateRequest};
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
