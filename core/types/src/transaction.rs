use std::collections::BTreeMap;
use std::net::IpAddr;

use anyhow::Context;
use ethers::types::Transaction as EthersTransaction;
use ethers::utils::rlp;
use fleek_crypto::{
    ConsensusPublicKey,
    EthAddress,
    NodePublicKey,
    TransactionSender,
    TransactionSignature,
};
use hp_fixed::unsigned::HpUfixed;
use ink_quill::{ToDigest, TranscriptBuilder, TranscriptBuilderInput};
use serde::{Deserialize, Serialize};

use super::{
    Epoch,
    Event,
    ProofOfConsensus,
    ProofOfMisbehavior,
    ReputationMeasurements,
    Service,
    ServiceId,
    Tokens,
};
use crate::content_registry::ContentUpdate;
use crate::{
    CommitteeSelectionBeaconCommit,
    CommitteeSelectionBeaconReveal,
    DeliveryAcknowledgmentProof,
    NodeIndex,
    NodePorts,
    ProtocolParamKey,
    ProtocolParamValue,
    TransactionDestination,
};

pub type ChainId = u32;

pub type TxHash = [u8; 32];

// TODO: Change this to capital and non-abrv version.
const FN_TXN_PAYLOAD_DOMAIN: &str = "fleek_network_txn_payload";

/// Create this wrapper so we can implement JsonSchema for EthersTransaction
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct EthersTransactionWrapper {
    pub tx: EthersTransaction,
}

impl schemars::JsonSchema for EthersTransactionWrapper {
    fn schema_name() -> String {
        "EthersTransaction".into()
    }

    fn schema_id() -> std::borrow::Cow<'static, str> {
        std::borrow::Cow::Borrowed(concat!(module_path!(), "::NodePublicKey"))
    }

    fn json_schema(_gen: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
        let tx = EthersTransaction::default();
        schemars::schema_for_value!(tx).schema.into()
    }
}

impl std::ops::Deref for EthersTransactionWrapper {
    type Target = EthersTransaction;

    fn deref(&self) -> &Self::Target {
        &self.tx
    }
}

impl std::ops::DerefMut for EthersTransactionWrapper {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.tx
    }
}

impl From<EthersTransaction> for EthersTransactionWrapper {
    fn from(tx: EthersTransaction) -> Self {
        Self { tx }
    }
}

impl From<EthersTransactionWrapper> for EthersTransaction {
    fn from(wrapper: EthersTransactionWrapper) -> Self {
        wrapper.tx
    }
}

/// A block of transactions, which is a list of update requests each signed by a user,
/// the block is the atomic view into the network, meaning that queries do not view
/// the intermediary state within a block, but only have the view to the latest executed
/// block.
#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub struct Block {
    /// Digest of the narwhal certificate that included this
    pub digest: [u8; 32],
    /// The narwhal subdag index that included these transactions
    pub sub_dag_index: u64,
    /// The narwhal subdag round that included these transactions
    pub sub_dag_round: u64,
    /// List of transactions to be executed in this block
    pub transactions: Vec<TransactionRequest>,
}

/// An update transaction, sent from users to the consensus to migrate the application
/// from one state to the next state.
#[derive(Debug, Hash, Clone, Serialize, Deserialize, Eq, PartialEq, schemars::JsonSchema)]
pub struct UpdateRequest {
    /// The signature by the user signing this payload.
    pub signature: TransactionSignature,
    /// The payload of an update request, which contains a counter (nonce), and
    /// the transition function itself.
    pub payload: UpdatePayload,
}

impl From<UpdateRequest> for TransactionRequest {
    fn from(value: UpdateRequest) -> Self {
        Self::UpdateRequest(value)
    }
}

impl From<EthersTransaction> for TransactionRequest {
    fn from(value: EthersTransaction) -> Self {
        Self::EthereumRequest(value.into())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, schemars::JsonSchema)]
pub enum TransactionRequest {
    UpdateRequest(UpdateRequest),
    EthereumRequest(EthersTransactionWrapper),
}

impl TransactionRequest {
    pub fn sender(&self) -> TransactionSender {
        match self {
            Self::UpdateRequest(payload) => payload.payload.sender,
            Self::EthereumRequest(payload) => EthAddress::from(payload.from.0).into(),
        }
    }

    pub fn hash(&self) -> TxHash {
        match self {
            Self::UpdateRequest(payload) => payload.payload.to_digest(),
            Self::EthereumRequest(payload) => payload.hash().0,
        }
    }

    pub fn to(&self) -> TransactionDestination {
        match self {
            Self::UpdateRequest(payload) => {
                TransactionDestination::Fleek(payload.payload.method.clone())
            },
            Self::EthereumRequest(payload) => {
                let address = if let Some(to_address) = payload.to {
                    to_address.0.into()
                } else {
                    [0; 20].into()
                };
                TransactionDestination::Ethereum(address)
            },
        }
    }

    pub fn chain_id(&self) -> ChainId {
        match self {
            Self::UpdateRequest(payload) => payload.payload.chain_id,
            Self::EthereumRequest(payload) => payload
                .chain_id
                .expect("Chain ID should be included in Ethereum Transaction")
                .as_u32(),
        }
    }

    pub fn event(&self) -> Option<Event> {
        if let TransactionSender::AccountOwner(sender) = self.sender() {
            match self {
                Self::UpdateRequest(payload) => match &payload.payload.method {
                    UpdateMethod::Transfer { amount, token, to } => Some(Event::transfer(
                        token.address(),
                        sender,
                        *to,
                        amount.clone(),
                    )),
                    UpdateMethod::SubmitDeliveryAcknowledgmentAggregation {
                        service_id,
                        metadata: event,
                        ..
                    } => event
                        .to_owned()
                        .map(|e| Event::service_event(*service_id, e)),
                    _ => None,
                },
                _ => None,
            }
        } else {
            None
        }
    }
}

impl TryFrom<&TransactionRequest> for Vec<u8> {
    type Error = anyhow::Error;

    fn try_from(value: &TransactionRequest) -> Result<Self, Self::Error> {
        match value {
            TransactionRequest::UpdateRequest(update_req) => {
                let mut bytes = bincode::serialize(update_req)?;
                bytes.push(0x00);
                Ok(bytes)
            },
            TransactionRequest::EthereumRequest(eth_tx) => {
                let mut bytes = eth_tx.rlp().to_vec();
                bytes.push(0x01);
                Ok(bytes)
            },
        }
    }
}

impl TryFrom<&[u8]> for TransactionRequest {
    type Error = anyhow::Error;

    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        let magic_byte = value[value.len() - 1];
        match magic_byte {
            0x00 => {
                let update_req = bincode::deserialize::<UpdateRequest>(&value[0..value.len() - 1])?;
                Ok(TransactionRequest::UpdateRequest(update_req))
            },
            0x01 => {
                let eth_tx = rlp::decode::<EthersTransaction>(&value[0..value.len() - 1])?;
                Ok(TransactionRequest::EthereumRequest(eth_tx.into()))
            },
            _ => Err(anyhow::anyhow!("Invalid magic byte: {magic_byte}")),
        }
    }
}

impl TryFrom<&Block> for Vec<u8> {
    type Error = anyhow::Error;

    fn try_from(value: &Block) -> Result<Self, Self::Error> {
        let mut bytes = Vec::new();
        // First 32 bytes are digest
        bytes.extend_from_slice(&value.digest);
        // Next 8 bytes are the subdag index
        bytes.extend_from_slice(&value.sub_dag_index.to_le_bytes());
        // Next 8 bytes are the subdag round
        bytes.extend_from_slice(&value.sub_dag_round.to_le_bytes());
        // Next 8 bytes are the number of transactions that are following
        let num_txns = value.transactions.len() as u64;
        bytes.extend_from_slice(&num_txns.to_le_bytes());
        // the rest of the bytes are the transactions
        for tx in &value.transactions {
            // TODO(matthias): would be good to serialize to borrowed bytes here instead
            let tx_bytes: Vec<u8> = tx.try_into()?;
            let tx_len = tx_bytes.len() as u64;
            bytes.extend_from_slice(&tx_len.to_le_bytes());
            bytes.extend_from_slice(&tx_bytes);
        }
        Ok(bytes)
    }
}

impl TryFrom<Vec<u8>> for Block {
    type Error = anyhow::Error;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        let digest: [u8; 32] = value.get(0..32).context("Out of bounds")?.try_into()?;

        let sub_dag_index_bytes = value.get(32..40).context("Out of bounds")?.try_into()?;
        let sub_dag_index = u64::from_le_bytes(sub_dag_index_bytes);

        let sub_dag_round_bytes = value.get(40..48).context("Out of bounds")?.try_into()?;
        let sub_dag_round = u64::from_le_bytes(sub_dag_round_bytes);

        let num_txns_bytes: [u8; 8] = value.get(48..56).context("Out of bounds")?.try_into()?;
        let num_txns = u64::from_le_bytes(num_txns_bytes);

        let mut pointer = 56;
        let mut transactions = Vec::with_capacity(num_txns as usize);
        for _ in 0..num_txns {
            let tx_len_bytes: [u8; 8] = value
                .get(pointer..pointer + 8)
                .context("Out of bounds")?
                .try_into()?;
            let tx_len = u64::from_le_bytes(tx_len_bytes) as usize;
            pointer += 8;
            let tx_bytes = value
                .get(pointer..pointer + tx_len)
                .context("Out of bounds")?;
            // TODO(matthias): can we get rid of this allocation?
            let tx: TransactionRequest = tx_bytes.try_into()?;
            transactions.push(tx);
            pointer += tx_len;
        }
        Ok(Block {
            digest,
            sub_dag_index,
            sub_dag_round,
            transactions,
        })
    }
}

/// The payload data of FN transaction
#[derive(Debug, Hash, Clone, Serialize, Deserialize, Eq, PartialEq, schemars::JsonSchema)]
pub struct UpdatePayload {
    /// The sender of the transaction.
    pub sender: TransactionSender,
    /// The counter or nonce of this request.
    pub nonce: u64,
    /// The transition function (and parameters) for this update request.
    pub method: UpdateMethod,
    /// The chain ID.
    pub chain_id: ChainId,
}

/// All of the update functions in our logic, along their parameters.
#[derive(Debug, Hash, Clone, Serialize, Deserialize, Eq, PartialEq, schemars::JsonSchema)]
pub enum UpdateMethod {
    /// The main function of the application layer. After aggregating ProofOfAcknowledgements a
    /// node will submit this transaction to get paid.
    /// Revisit the naming of this transaction.
    SubmitDeliveryAcknowledgmentAggregation {
        /// How much of the commodity was served
        commodity: u128,
        /// The service id of the service this was provided through(CDN, compute, ect.)
        service_id: u32,
        /// The PoD of delivery in bytes
        proofs: Vec<DeliveryAcknowledgmentProof>,
        /// Optional metadata to provide information additional information about this batch
        metadata: Option<Vec<u8>>,
    },
    /// Withdraw tokens from the network back to the L2
    Withdraw {
        /// The amount to withdraw.
        amount: HpUfixed<18>,
        /// Which token to withdraw.
        token: Tokens,
        /// The address to recieve these tokens on the L2
        receiving_address: EthAddress,
    },
    /// Submit of PoC from the bridge on the L2 to get the tokens in network
    Deposit {
        /// The proof of the bridge recieved from the L2,
        proof: ProofOfConsensus,
        /// Which token was bridged
        token: Tokens,
        /// Amount bridged
        amount: HpUfixed<18>,
    },
    /// Transfer tokens to another address
    Transfer {
        /// The amount to transfer
        amount: HpUfixed<18>,
        /// Which token to transfer
        token: Tokens,
        /// The address to transfer to
        to: EthAddress,
    },
    /// Stake FLK in network
    Stake {
        /// Amount to stake
        amount: HpUfixed<18>,
        /// Node Public Key
        node_public_key: NodePublicKey,
        /// Consensus Public Key
        consensus_key: Option<ConsensusPublicKey>,
        /// Nodes primary internet address
        node_domain: Option<IpAddr>,
        /// Worker public Key
        worker_public_key: Option<NodePublicKey>,
        /// internet address for the worker
        worker_domain: Option<IpAddr>,
        /// internet address for workers mempool
        ports: Option<NodePorts>,
    },
    /// Lock the current stakes to get boosted inflation rewards
    /// this is different than unstake and withdrawl lock
    StakeLock {
        node: NodePublicKey,
        locked_for: u64,
    },
    /// Unstake FLK, the tokens will be locked for a set amount of
    /// time(ProtocolParameter::LockTime) before they can be withdrawn
    Unstake {
        amount: HpUfixed<18>,
        node: NodePublicKey,
    },
    /// Withdraw tokens from a node after lock period has passed
    /// must be submitted by node owner but optionally they can provide a different public key to
    /// receive the tokens
    WithdrawUnstaked {
        node: NodePublicKey,
        recipient: Option<EthAddress>,
    },
    /// Sent by committee member to signal he is ready to change epoch
    ChangeEpoch { epoch: Epoch },
    /// Sent by nodes to commit their committee selection random beacon.
    CommitteeSelectionBeaconCommit {
        commit: CommitteeSelectionBeaconCommit,
    },
    /// Sent by nodes to reveal their committee selection random beacon.
    CommitteeSelectionBeaconReveal {
        reveal: CommitteeSelectionBeaconReveal,
    },
    /// Sent by nodes when they see that the committee selection beacon commit phase has timed out.
    CommitteeSelectionBeaconCommitPhaseTimeout,
    /// Sent by nodes when they see that the committee selection beacon reveal phase has timed out.
    CommitteeSelectionBeaconRevealPhaseTimeout,
    /// Adding a new service to the protocol
    AddService {
        service: Service,
        service_id: ServiceId,
    },
    /// Removing a service from the protocol
    RemoveService {
        /// Service Id of the service to be removed
        service_id: ServiceId,
    },
    /// Provide proof of misbehavior to slash a node
    Slash {
        /// Service id of the service a node misbehaved in
        service_id: ServiceId,
        /// The public key of the node that misbehaved
        node: NodePublicKey,
        /// Zk proof to be provided to the slash circuit
        proof_of_misbehavior: ProofOfMisbehavior,
    },
    /// Report reputation measurements
    SubmitReputationMeasurements {
        measurements: BTreeMap<NodeIndex, ReputationMeasurements>,
    },
    /// Change protocol parameters
    ChangeProtocolParam {
        param: ProtocolParamKey,
        value: ProtocolParamValue,
    },
    /// Opt out of participating in the network.
    OptOut {},
    /// Opt into participating in the network.
    OptIn {},
    /// Update the content registry.
    ///
    /// The content registry records the contents that are being
    /// provided by the network and the corresponding nodes that
    /// are providing that content.
    UpdateContentRegistry { updates: Vec<ContentUpdate> },
    /// Increment the node nonce.
    IncrementNonce {},
}

impl ToDigest for UpdatePayload {
    /// Computes the hash of this update payload and returns a 32-byte hash
    /// that can be signed by the user.
    ///
    /// # Safety
    ///
    /// This function must take all of the data into account, including the
    /// nonce, the name of all of the update method names along with the value
    /// for all of the parameters.
    fn transcript(&self) -> TranscriptBuilder {
        let mut transcript_builder = TranscriptBuilder::empty(FN_TXN_PAYLOAD_DOMAIN)
            .with("nonce", &self.nonce)
            .with("chain_id", &self.chain_id);

        match &self.sender {
            TransactionSender::NodeMain(public_key) => {
                transcript_builder = transcript_builder.with("sender_node", &public_key.0);
            },
            TransactionSender::AccountOwner(address) => {
                transcript_builder = transcript_builder.with("sender_account", &address.0);
            },
            TransactionSender::NodeConsensus(public_key) => {
                transcript_builder = transcript_builder.with("sender_consensus", &public_key.0);
            },
        }

        // insert method fields
        match &self.method {
            UpdateMethod::Deposit {
                proof: _,
                token,
                amount,
            } => {
                transcript_builder = transcript_builder
                    .with("transaction_name", &"deposit")
                    .with_prefix("input".to_owned())
                    .with("token", token)
                    .with("amount", &HpUfixedWrapper(amount.clone()));
                //.with("method.proof", proof);
            },

            UpdateMethod::SubmitDeliveryAcknowledgmentAggregation {
                commodity,
                service_id,
                proofs: _,
                metadata,
            } => {
                transcript_builder = transcript_builder
                    .with(
                        "transaction_name",
                        &"submit_delivery_acknowledgment_aggregation",
                    )
                    .with_prefix("input".to_owned())
                    .with("commodity", commodity)
                    .with("service_id", service_id)
                    .with("metadata", metadata);
                //.with("method.proof", proof);
            },
            UpdateMethod::Withdraw {
                amount,
                token,
                receiving_address,
            } => {
                transcript_builder = transcript_builder
                    .with("transaction_name", &"withdraw")
                    .with_prefix("input".to_owned())
                    .with("amount", &HpUfixedWrapper(amount.clone()))
                    .with("token", token)
                    .with("receiving_address", &receiving_address.0);
            },
            UpdateMethod::Transfer { amount, token, to } => {
                transcript_builder = transcript_builder
                    .with("transaction_name", &"transfer")
                    .with_prefix("input".to_owned())
                    .with("token", token)
                    .with("amount", &HpUfixedWrapper(amount.clone()))
                    .with("to", &to.0);
            },
            UpdateMethod::Stake {
                amount,
                node_public_key,
                consensus_key,
                node_domain,
                worker_public_key,
                worker_domain,
                ports,
            } => {
                transcript_builder = transcript_builder
                    .with("transaction_name", &"stake")
                    .with_prefix("input".to_owned())
                    .with("amount", &HpUfixedWrapper(amount.clone()))
                    .with("node_public_key", &node_public_key.0)
                    .with(
                        "node_network_key",
                        &consensus_key.map_or([0u8; 96], |key| key.0),
                    )
                    .with("node_domain", &node_domain.map(|d| d.to_string()))
                    .with(
                        "worker_public_key",
                        &worker_public_key.map_or([0u8; 32], |key| key.0),
                    )
                    .with("worker_domain", &worker_domain.map(|d| d.to_string()))
                    .with("primary_port", &ports.as_ref().map(|p| p.primary))
                    .with("worker_port", &ports.as_ref().map(|p| p.worker))
                    .with("mempool_port", &ports.as_ref().map(|p| p.mempool))
                    .with("rpc_port", &ports.as_ref().map(|p| p.rpc))
                    .with("pool_port", &ports.as_ref().map(|p| p.pool))
                    .with("pinger_port", &ports.as_ref().map(|p| p.pinger))
                    .with(
                        "handshake_http_port",
                        &ports.as_ref().map(|p| p.handshake.http),
                    )
                    .with(
                        "handshake_webrtc_port",
                        &ports.as_ref().map(|p| p.handshake.webrtc),
                    )
                    .with(
                        "handshake_webtransport_port",
                        &ports.as_ref().map(|p| p.handshake.webtransport),
                    )
            },
            UpdateMethod::StakeLock { node, locked_for } => {
                transcript_builder = transcript_builder
                    .with("transaction_name", &"stake_lock")
                    .with_prefix("input".to_owned())
                    .with("node", &node.0)
                    .with("locked_for", locked_for);
            },
            UpdateMethod::Unstake { amount, node } => {
                transcript_builder = transcript_builder
                    .with("transaction_name", &"unstake")
                    .with_prefix("input".to_owned())
                    .with("node", &node.0)
                    .with("amount", &HpUfixedWrapper(amount.clone()));
            },
            UpdateMethod::WithdrawUnstaked { node, recipient } => {
                transcript_builder = transcript_builder
                    .with("transaction_name", &"withdraw_unstaked")
                    .with_prefix("input".to_owned())
                    .with("node", &node.0)
                    .with("recipient", &recipient.map_or([0u8; 20], |key| key.0));
            },
            UpdateMethod::ChangeEpoch { epoch } => {
                transcript_builder = transcript_builder
                    .with("transaction_name", &"change_epoch")
                    .with_prefix("input".to_owned())
                    .with("epoch", epoch);
            },
            UpdateMethod::CommitteeSelectionBeaconCommit { commit } => {
                transcript_builder = transcript_builder
                    .with("transaction_name", &"committee_selection_beacon_commit")
                    .with("commit", &commit.hash)
                    .with_prefix("input".to_owned());
            },
            UpdateMethod::CommitteeSelectionBeaconReveal { reveal } => {
                transcript_builder = transcript_builder
                    .with("transaction_name", &"committee_selection_beacon_reveal")
                    .with("reveal", reveal)
                    .with_prefix("input".to_owned());
            },
            UpdateMethod::CommitteeSelectionBeaconCommitPhaseTimeout => {
                transcript_builder = transcript_builder.with(
                    "transaction_name",
                    &"committee_selection_beacon_commit_phase_timeout",
                );
            },
            UpdateMethod::CommitteeSelectionBeaconRevealPhaseTimeout => {
                transcript_builder = transcript_builder.with(
                    "transaction_name",
                    &"committee_selection_beacon_reveal_phase_timeout",
                );
            },
            UpdateMethod::AddService {
                service,
                service_id,
            } => {
                transcript_builder = transcript_builder
                    .with("transaction_name", &"add_service")
                    .with_prefix("input".to_owned())
                    .with("service_id", service_id)
                    .with("service", service);
            },
            UpdateMethod::RemoveService { service_id } => {
                transcript_builder = transcript_builder
                    .with("transaction_name", &"remove_service")
                    .with_prefix("input".to_owned())
                    .with("service_id", service_id);
            },
            UpdateMethod::Slash {
                service_id,
                node,
                proof_of_misbehavior: _,
            } => {
                transcript_builder = transcript_builder
                    .with("transaction_name", &"slash")
                    .with_prefix("input".to_owned())
                    .with("service_id", service_id)
                .with("node", &node.0)
                // .with("proof_of_misbehavior", proof_of_misbehavior)
                ;
            },
            UpdateMethod::SubmitReputationMeasurements { measurements } => {
                transcript_builder =
                    transcript_builder.with("transaction_name", &"submit_reputation_measurements");
                for (key, value) in measurements {
                    transcript_builder = transcript_builder
                        .with_prefix(key.to_string())
                        .with("latency", &value.latency.map_or(0, |l| l.as_nanos()))
                        .with("interactions", &value.interactions)
                        .with("inbound_bandwidth", &value.inbound_bandwidth)
                        .with("outbound_bandwidth", &value.outbound_bandwidth)
                        .with("bytes_received", &value.bytes_received)
                        .with("bytes_sent", &value.bytes_sent)
                        .with("hops", &value.hops);
                }
            },
            UpdateMethod::ChangeProtocolParam { param, value } => {
                transcript_builder = transcript_builder
                    .with("transaction_name", &"change_protocol_param")
                    .with_prefix("input".to_owned())
                    .with("param", &(param.clone() as u8))
                    .with("value", &value.get_bytes().as_ref());
            },
            UpdateMethod::OptIn {} => {
                transcript_builder = transcript_builder.with("transaction_name", &"opt_in");
            },
            UpdateMethod::OptOut {} => {
                transcript_builder = transcript_builder.with("transaction_name", &"opt_out");
            },
            UpdateMethod::UpdateContentRegistry { updates } => {
                transcript_builder =
                    transcript_builder.with("transaction_name", &"update_content_registry");
                for (idx, update) in updates.iter().enumerate() {
                    transcript_builder = transcript_builder
                        .with_prefix(idx.to_string())
                        .with("uri", &update.uri)
                        .with("remove", &(update.remove as u8));
                }
            },
            UpdateMethod::IncrementNonce {} => {
                transcript_builder = transcript_builder.with("transaction_name", &"inc_nonce");
            },
        }

        transcript_builder
    }
}

struct HpUfixedWrapper<const T: usize>(HpUfixed<T>);

impl<const T: usize> HpUfixedWrapper<T> {
    #[inline]
    pub fn get_value(&self) -> &ruint::aliases::U256 {
        self.0.get_value()
    }
}

impl<const P: usize> TranscriptBuilderInput for HpUfixedWrapper<P> {
    const TYPE: &'static str = "HpUfixed";

    fn to_transcript_builder_input(&self) -> Vec<u8> {
        let mut input = Vec::new();

        // Append the precision value as a byte
        input.push(P as u8);

        // Append the BigUint data as bytes
        let data_bytes: [u8; 32] = self.get_value().to_le_bytes();
        input.extend_from_slice(&data_bytes);

        input
    }
}

#[cfg(test)]
mod tests {
    use ethers::types::U256;
    use fleek_crypto::{AccountOwnerSignature, NodeSignature};

    use super::*;

    const CHAIN_ID: ChainId = 1337;

    #[test]
    fn test_transaction_request_eth() {
        let tx = EthersTransaction {
            chain_id: Some(U256::from(CHAIN_ID)),
            ..Default::default()
        };
        let tx_req = TransactionRequest::EthereumRequest(tx.into());

        assert_eq!(tx_req.chain_id(), CHAIN_ID);

        let bytes: Vec<u8> = (&tx_req).try_into().unwrap();
        let tx_req_r = TransactionRequest::try_from(bytes.as_ref()).unwrap();

        let bytes: Vec<u8> = (&tx_req_r).try_into().unwrap();
        let tx_req_rr = TransactionRequest::try_from(bytes.as_ref()).unwrap();
        assert_eq!(tx_req_r, tx_req_rr);
    }

    #[test]
    fn test_transaction_request_fleek() {
        let update_method = UpdateMethod::ChangeProtocolParam {
            param: ProtocolParamKey::CommitteeSize,
            value: ProtocolParamValue::CommitteeSize(4),
        };
        let payload = UpdatePayload {
            sender: TransactionSender::AccountOwner(EthAddress([0; 20])),
            nonce: 0,
            method: update_method,
            chain_id: CHAIN_ID,
        };
        let update_req = UpdateRequest {
            signature: TransactionSignature::AccountOwner(AccountOwnerSignature([0; 65])),
            payload,
        };
        let tx = TransactionRequest::UpdateRequest(update_req);
        let bytes: Vec<u8> = (&tx).try_into().unwrap();
        let tx_r = TransactionRequest::try_from(bytes.as_ref()).unwrap();
        assert_eq!(tx, tx_r);
    }

    #[test]
    fn test_update_payload_hash_includes_chain_id() {
        let chain_id_1 = CHAIN_ID;
        let chain_id_2 = chain_id_1 + 1;

        let update_method = UpdateMethod::ChangeProtocolParam {
            param: ProtocolParamKey::CommitteeSize,
            value: ProtocolParamValue::CommitteeSize(4),
        };
        let payload_1 = UpdatePayload {
            sender: TransactionSender::AccountOwner(EthAddress([0; 20])),
            nonce: 0,
            method: update_method,
            chain_id: chain_id_1,
        };

        let mut payload_2 = payload_1.clone();
        payload_2.chain_id = chain_id_2;

        assert_ne!(payload_1.to_digest(), payload_2.to_digest());

        let update_1 = UpdateRequest {
            signature: TransactionSignature::AccountOwner(AccountOwnerSignature([0; 65])),
            payload: payload_1,
        };
        let tx_1 = TransactionRequest::UpdateRequest(update_1);
        assert_eq!(tx_1.chain_id(), chain_id_1);

        let update_2 = UpdateRequest {
            signature: TransactionSignature::AccountOwner(AccountOwnerSignature([0; 65])),
            payload: payload_2,
        };
        let tx_2 = TransactionRequest::UpdateRequest(update_2);
        assert_eq!(tx_2.chain_id(), chain_id_2)
    }

    #[test]
    fn test_block_to_bytes_from_bytes() {
        let txn = TransactionRequest::UpdateRequest(UpdateRequest {
            signature: TransactionSignature::NodeMain(NodeSignature([9; 64])),
            payload: UpdatePayload {
                sender: TransactionSender::NodeMain(NodePublicKey([9; 32])),
                nonce: 0,
                method: UpdateMethod::ChangeEpoch { epoch: 0 },
                chain_id: 69,
            },
        });
        let block = Block {
            digest: [2; 32],
            sub_dag_index: 0,
            sub_dag_round: 0,
            transactions: vec![txn.clone(), txn.clone(), txn.clone()],
        };

        let block_bytes = <Vec<u8>>::try_from(&block).unwrap();
        let new_block = <Block>::try_from(block_bytes).unwrap();

        assert_eq!(block, new_block);
    }
}
