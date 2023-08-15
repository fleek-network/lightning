use std::collections::BTreeMap;

use fleek_crypto::{
    ConsensusPublicKey, EthAddress, NodePublicKey, TransactionSender, TransactionSignature,
};
use hp_fixed::unsigned::HpUfixed;
use ink_quill::{ToDigest, TranscriptBuilder, TranscriptBuilderInput};
use num_bigint::BigUint;
use serde::{Deserialize, Serialize};

use super::{
    DeliveryAcknowledgment, Epoch, ProofOfConsensus, ProofOfMisbehavior, ProtocolParams,
    ReputationMeasurements, Service, ServiceId, Tokens,
};
use crate::NodeIndex;

// TODO: Change this to capital and non-abrv version.
const FN_TXN_PAYLOAD_DOMAIN: &str = "fleek_network_txn_payload";

/// A block of transactions, which is a list of update requests each signed by a user,
/// the block is the atomic view into the network, meaning that queries do not view
/// the intermediary state within a block, but only have the view to the latest executed
/// block.
#[derive(Debug)]
pub struct Block {
    pub transactions: Vec<UpdateRequest>,
}

/// An update transaction, sent from users to the consensus to migrate the application
/// from one state to the next state.
#[derive(Debug, Hash, Clone, Serialize, Deserialize)]
pub struct UpdateRequest {
    /// The sender of the transaction.
    pub sender: TransactionSender,
    /// The signature by the user signing this payload.
    pub signature: TransactionSignature,
    /// The payload of an update request, which contains a counter (nonce), and
    /// the transition function itself.
    pub payload: UpdatePayload,
}

/// The payload data of an update request.
#[derive(Debug, Hash, Clone, Serialize, Deserialize)]
pub struct UpdatePayload {
    /// The counter or nonce of this request.
    pub nonce: u64,
    /// The transition function (and parameters) for this update request.
    pub method: UpdateMethod,
}

/// All of the update functions in our logic, along their parameters.
#[derive(Debug, Hash, Clone, Serialize, Deserialize)]
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
        proofs: Vec<DeliveryAcknowledgment>,
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
    /// Stake FLK in network
    Stake {
        /// Amount to stake
        amount: HpUfixed<18>,
        /// Node Public Key
        node_public_key: NodePublicKey,
        /// Consensus Public Key
        consensus_key: Option<ConsensusPublicKey>,
        /// Nodes primary internet address
        node_domain: Option<String>,
        /// Worker public Key
        worker_public_key: Option<NodePublicKey>,
        /// internet address for the worker
        worker_domain: Option<String>,
        /// internet address for workers mempool
        worker_mempool_address: Option<String>,
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
    ChangeEpoch {
        epoch: Epoch,
    },
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
    SubmitReputationMeasurements {
        measurements: BTreeMap<NodeIndex, ReputationMeasurements>,
    },
    ChangeProtocolParam {
        param: ProtocolParams,
        value: u128,
    },
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
        let mut transcript_builder =
            TranscriptBuilder::empty(FN_TXN_PAYLOAD_DOMAIN).with("nonce", &self.nonce);

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
            UpdateMethod::Stake {
                amount,
                node_public_key,
                consensus_key,
                node_domain,
                worker_public_key,
                worker_domain,
                worker_mempool_address,
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
                    .with("node_domain", node_domain)
                    .with(
                        "worker_public_key",
                        &worker_public_key.map_or([0u8; 32], |key| key.0),
                    )
                    .with("worker_domain", worker_domain)
                    .with("worker_mempool_address", worker_mempool_address);
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
                    .with("value", value);
            },
        }

        transcript_builder
    }
}

struct HpUfixedWrapper<const T: usize>(HpUfixed<T>);

impl<const T: usize> HpUfixedWrapper<T> {
    #[inline]
    pub fn get_value(&self) -> &BigUint {
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
        let data_bytes = self.get_value().to_bytes_le();
        input.extend_from_slice(&data_bytes);

        input
    }
}
