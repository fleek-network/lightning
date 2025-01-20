use ethers::prelude::abigen;
use ethers::types::transaction::eip2718::TypedTransaction;
use ethers::types::Transaction as EthersTransaction;
use fleek_crypto::EthAddress;

abigen!(
    FleekContract,
    r"[
        function withdraw(uint256 amount, string token, address recipient)
        function stake(uint256 amount, bytes32 nodePublicKey, bytes consensusKey, string domain, bytes32 workerPublicKey, string workerDomain)
        function deposit(string token, uint256 amount)
        function unstake(uint256 amount, bytes32 node_public_key)
        function withdrawUnstaked(bytes32 node_public_key, address recipient)
        function approveClientKey(bytes client_key)
        function revokeClientKey()
    ]"
);

pub fn verify_signature(txn: &EthersTransaction, sender: EthAddress) -> bool {
    let typed_txn: TypedTransaction = txn.into();
    let mut sig = [0u8; 65];
    txn.r.to_big_endian(&mut sig[0..32]);
    txn.s.to_big_endian(&mut sig[32..64]);
    sig[64] = normalize_recovery_id(txn.v.as_u64());
    sender.verify(&sig.into(), typed_txn.rlp().as_ref())
}

fn normalize_recovery_id(v: u64) -> u8 {
    match v {
        0 => 0,
        1 => 1,
        27 => 0,
        28 => 1,
        v if v >= 35 => ((v - 1) % 2) as _,
        _ => 4,
    }
}
