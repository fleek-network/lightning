use draco_interfaces::{
    identity::{BlsPublicKey, Ed25519PublicKey, EthPublicKey, PeerId},
    types::{
        Epoch, InternetAddress, Metadata, ProofOfConsensus, ProofOfMisbehavior, ProtocolParams,
        QueryMethod, QueryRequest, Service, ServiceId, Tokens, UpdateMethod, UpdateRequest,
    },
    DeliveryAcknowledgment,
};
use multiaddr::Multiaddr;
use serde::{Deserialize, Serialize};

use crate::table::{Backend, TableRef};

/// The state of the Application
///
/// The functions implemented on this struct are the "Smart Contracts" of the application layer
/// All state changes come from Transactions and start at execute_txn
pub struct State<B: Backend> {
    pub metadata: B::Ref<Metadata, u64>,
    pub account_info: B::Ref<Ed25519PublicKey, AccountInfo>,
    pub node_info: B::Ref<BlsPublicKey, NodeInfo>,
    pub committee_info: B::Ref<Epoch, Committee>,
    pub bandwidth_info: B::Ref<Epoch, BandwidthInfo>,
    pub services: B::Ref<ServiceId, Service>,
    pub parameters: B::Ref<ProtocolParams, u128>,
    pub backend: B,
}

/// The account info stored per account on the blockchain
#[derive(Debug, Hash, PartialEq, PartialOrd, Ord, Eq, Serialize, Deserialize, Clone, Default)]
pub struct AccountInfo {
    pub flk_balance: u128,
    pub bandwidth_balance: u128,
    pub nonce: u128,
    pub staking: Staking,
}

/// Struct that stores
#[derive(Debug, Hash, PartialEq, PartialOrd, Ord, Eq, Serialize, Deserialize, Clone, Default)]
pub struct Staking {
    pub staked: u128,
    pub locked: u128,
    pub locked_until: u64,
}

#[derive(Debug, Hash, PartialEq, PartialOrd, Ord, Eq, Serialize, Deserialize, Clone)]
pub struct Committee {
    pub members: Vec<BlsPublicKey>,
    pub ready_to_change: Vec<BlsPublicKey>,
    pub epoch_end_timestamp: u64,
}

#[derive(Debug, Hash, PartialEq, PartialOrd, Ord, Eq, Serialize, Deserialize, Clone)]
pub struct BandwidthInfo {
    pub total_served: u128,
    pub reward_pool: u128,
}

#[derive(Debug, Hash, PartialEq, PartialOrd, Ord, Eq, Serialize, Deserialize, Clone)]
pub struct NodeInfo {
    pub owner: Ed25519PublicKey,
    pub public_key: BlsPublicKey,
    pub network_key: Ed25519PublicKey,
    pub domain: Multiaddr,
    pub workers: Vec<Worker>,
}

#[derive(Clone, Default, Serialize, Deserialize, Debug)]
pub struct EpochInfo {
    pub committee: Vec<NodeInfo>,
    pub epoch: Epoch,
    pub epoch_end: u64,
}

#[derive(Debug, Hash, PartialEq, PartialOrd, Ord, Eq, Serialize, Deserialize, Clone)]
pub struct Worker {
    pub public_key: Ed25519PublicKey,
    pub address: Multiaddr,
    pub mempool: Multiaddr,
}

#[derive(Clone, Debug)]
pub enum TransactionResponse {
    Success(ExecutionData),
    Revert(ExecutionError),
}

impl TransactionResponse {
    /// If response contains a Uint will return
    pub fn to_number(&self) -> Result<u128, TransactionResponse> {
        if let TransactionResponse::Success(ExecutionData::UInt(num)) = self {
            Ok(*num)
        } else {
            Err(self.clone())
        }
    }
}

#[derive(Clone, Debug)]
pub enum ExecutionData {
    None,
    String(String),
    UInt(u128),
    EpochInfo(EpochInfo),
    EpochChange,
}

#[derive(Clone, Debug)]
pub enum ExecutionError {
    InvalidSignature,
    InvalidNonce,
    InvalidProof,
    NotNodeOwner,
    NotCommitteeMember,
    NodeDoesNotExist,
    AlreadySignaled,
    NonExistingService,
}

impl<B: Backend> State<B> {
    pub fn new(backend: B) -> Self {
        Self {
            metadata: backend.get_table_reference("metadata"),
            account_info: backend.get_table_reference("account"),
            node_info: backend.get_table_reference("node"),
            committee_info: backend.get_table_reference("committee"),
            bandwidth_info: backend.get_table_reference("bandwidth"),
            services: backend.get_table_reference("service"),
            parameters: backend.get_table_reference("parameter"),
            backend,
        }
    }

    /// This function is the entry point of a transaction
    pub fn execute_txn(&self, txn: UpdateRequest) -> TransactionResponse {
        match txn.payload.method {
            UpdateMethod::SubmitDeliveryAcknowledgmentAggregation {
                commodity,
                service_id,
                proofs,
                metadata: _,
            } => self.submit_pod(txn.sender, commodity, service_id, proofs),

            UpdateMethod::Withdraw {
                amount,
                token,
                receiving_address,
            } => self.withdraw(txn.sender, receiving_address, amount, token),

            UpdateMethod::Deposit {
                proof,
                token,
                amount,
            } => self.deposit(txn.sender, proof, amount, token),

            UpdateMethod::Stake {
                proof,
                amount,
                node,
            } => self.stake(txn.sender, proof, amount, node),

            UpdateMethod::Unstake { amount } => self.unstake(txn.sender, amount),

            UpdateMethod::ChangeEpoch => self.change_epoch(txn.sender),

            UpdateMethod::AddService {
                service,
                service_id,
            } => self.add_service(txn.sender, service, service_id),

            UpdateMethod::RemoveService { service_id } => {
                self.remove_service(txn.sender, service_id)
            },

            UpdateMethod::Slash {
                service_id,
                node,
                proof_of_misbehavior,
            } => self.slash(txn.sender, proof_of_misbehavior, service_id, node),
            // TransactionType::Query(query) => match query {
            //     Query::FLK { public_key } => self.get_flk(public_key),
            //     Query::Locked { public_key } => self.get_locked(public_key),
            //     Query::Bandwidth { public_key } => self.get_bandwidth(public_key),
            //     Query::Served { epoch, node } => self.get_node_bandwidth_served(epoch, node),
            //     Query::RewardPool { epoch } => self.get_reward_pool(epoch),
            //     Query::TotalServed { epoch } => self.get_total_served(epoch),
            //     Query::Staked { node } => self.get_staked(node),
            //     Query::CurrentEpochInfo => self.get_current_epoch_info(),
            // },
        }
    }

    pub fn execute_query(&self, txn: QueryRequest) -> TransactionResponse {
        match txn.query {
            QueryMethod::Bandwidth { public_key } => todo!(),
            QueryMethod::FLK { public_key } => todo!(),
            QueryMethod::Locked { public_key } => todo!(),
            QueryMethod::Staked { node } => todo!(),
            QueryMethod::Served { epoch, node } => todo!(),
            QueryMethod::TotalServed { epoch } => todo!(),
            QueryMethod::RewardPool { epoch } => todo!(),
            QueryMethod::CurrentEpochInfo => todo!(),
        }
    }
    /*********** External Update Functions ********** */
    // The following functions should only be called in the result of a query or update transaction
    // through execute_txn() If called in an update txn it will mutate state
    fn submit_pod(
        &self,
        sender: PeerId,
        commodity: u128,
        service_id: u64,
        acknowledgments: Vec<DeliveryAcknowledgment>,
    ) -> TransactionResponse {
        todo!()
    }

    fn withdraw(
        &self,
        sender: PeerId,
        reciever: EthPublicKey,
        amount: u128,
        token: Tokens,
    ) -> TransactionResponse {
        todo!()
    }

    fn deposit(
        &self,
        sender: PeerId,
        proof: ProofOfConsensus,
        amount: u128,
        token: Tokens,
    ) -> TransactionResponse {
        todo!()
    }

    fn stake(
        &self,
        sender: PeerId,
        proof: ProofOfConsensus,
        amount: u128,
        node: PeerId,
    ) -> TransactionResponse {
        todo!()
    }

    fn unstake(&self, sender: PeerId, amount: u128) -> TransactionResponse {
        todo!()
    }

    fn change_epoch(&self, sender: PeerId) -> TransactionResponse {
        todo!()
    }

    fn add_service(
        &self,
        sender: PeerId,
        service: Service,
        service_id: ServiceId,
    ) -> TransactionResponse {
        todo!()
    }

    fn remove_service(&self, sender: PeerId, service_id: ServiceId) -> TransactionResponse {
        todo!()
    }

    fn slash(
        &self,
        sender: PeerId,
        proof: ProofOfMisbehavior,
        service_id: ServiceId,
        node: BlsPublicKey,
    ) -> TransactionResponse {
        todo!()
    }

    /*******External View Functions****** */
    // The following functions should be called through execute_txn as the result of a txn
    // They will never change state even if called through update
    // Will usually only be called through query calls where msg.sender is not checked
    //      so if that is required for the function it should be made a parameter instead

    fn get_flk(&self, account: PeerId) -> TransactionResponse {
        todo!()
    }
    fn get_locked(&self, account: PeerId) -> TransactionResponse {
        todo!()
    }
    fn get_bandwidth(&self, account: PeerId) -> TransactionResponse {
        todo!()
    }
    fn get_staked(&self, node: BlsPublicKey) -> TransactionResponse {
        todo!()
    }
    fn get_reward_pool(&self, epoch: Epoch) -> TransactionResponse {
        todo!()
    }
    fn get_total_served(&self, epoch: Epoch) -> TransactionResponse {
        todo!()
    }
    fn get_node_bandwidth_served(&self, epoch: Epoch, node: PeerId) -> TransactionResponse {
        todo!()
    }
    fn get_current_epoch_info(&self) -> TransactionResponse {
        todo!()
    }
    /********Internal Application Functions******** */
    // These functions should only ever be called in the context of an external transaction function
    // They should never panic and any check that could result in that should be done in the
    // external function that calls it The functions that should call this and the required
    // checks should be documented for each function

    // This function should be called during signal_epoch_change.
    fn distribute_rewards(&self) {
        todo!()
    }
    fn choose_new_committee(&self) -> Vec<BlsPublicKey> {
        todo!()
    }
}
