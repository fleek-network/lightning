use draco_interfaces::{
    types::{
        AccountInfo, Epoch, Metadata, NodeInfo, ProofOfConsensus, ProofOfMisbehavior,
        ProtocolParams, QueryMethod, QueryRequest, Service, ServiceId, Tokens, TransactionResponse,
        UpdateMethod, UpdateRequest,
    },
    DeliveryAcknowledgment,
};
use fleek_crypto::{AccountOwnerPublicKey, NodePublicKey, TransactionSender};
use serde::{Deserialize, Serialize};

use crate::table::Backend;

/// The state of the Application
///
/// The functions implemented on this struct are the "Smart Contracts" of the application layer
/// All state changes come from Transactions and start at execute_txn
pub struct State<B: Backend> {
    pub metadata: B::Ref<Metadata, u64>,
    pub account_info: B::Ref<AccountOwnerPublicKey, AccountInfo>,
    pub node_info: B::Ref<NodePublicKey, NodeInfo>,
    pub committee_info: B::Ref<Epoch, Committee>,
    pub bandwidth_info: B::Ref<Epoch, BandwidthInfo>,
    pub services: B::Ref<ServiceId, Service>,
    pub parameters: B::Ref<ProtocolParams, u128>,
    pub backend: B,
}

#[derive(Debug, Hash, PartialEq, PartialOrd, Ord, Eq, Serialize, Deserialize, Clone)]
pub struct Committee {
    pub members: Vec<NodePublicKey>,
    pub ready_to_change: Vec<NodePublicKey>,
    pub epoch_end_timestamp: u64,
}

#[derive(Debug, Hash, PartialEq, PartialOrd, Ord, Eq, Serialize, Deserialize, Clone)]
pub struct BandwidthInfo {
    pub total_served: u128,
    pub reward_pool: u128,
}

#[derive(Clone, Default, Serialize, Deserialize, Debug)]
pub struct EpochInfo {
    pub committee: Vec<NodeInfo>,
    pub epoch: Epoch,
    pub epoch_end: u64,
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
        }
    }

    pub fn execute_query(&self, txn: QueryRequest) -> TransactionResponse {
        match txn.query {
            QueryMethod::Bandwidth { public_key } => self.get_bandwidth(public_key),
            QueryMethod::FLK { public_key } => self.get_flk(public_key),
            QueryMethod::Locked { public_key } => self.get_locked(public_key),
            QueryMethod::Staked { node } => self.get_staked(node),
            QueryMethod::Served { epoch, node } => self.get_node_bandwidth_served(epoch, node),
            QueryMethod::TotalServed { epoch } => self.get_total_served(epoch),
            QueryMethod::RewardPool { epoch } => self.get_reward_pool(epoch),
            QueryMethod::CurrentEpochInfo => self.get_current_epoch_info(),
        }
    }
    /*********** External Update Functions ********** */
    // The following functions should only be called in the result of a query or update transaction
    // through execute_txn() If called in an update txn it will mutate state
    fn submit_pod(
        &self,
        _sender: TransactionSender,
        _commodity: u128,
        _service_id: u64,
        _acknowledgments: Vec<DeliveryAcknowledgment>,
    ) -> TransactionResponse {
        todo!()
    }

    fn withdraw(
        &self,
        _sender: TransactionSender,
        _reciever: AccountOwnerPublicKey,
        _amount: u128,
        _token: Tokens,
    ) -> TransactionResponse {
        todo!()
    }

    fn deposit(
        &self,
        _sender: TransactionSender,
        _proof: ProofOfConsensus,
        _amount: u128,
        _token: Tokens,
    ) -> TransactionResponse {
        todo!()
    }

    fn stake(
        &self,
        _sender: TransactionSender,
        _proof: ProofOfConsensus,
        _amount: u128,
        _node: NodePublicKey,
    ) -> TransactionResponse {
        todo!()
    }

    fn unstake(&self, _sender: TransactionSender, _amount: u128) -> TransactionResponse {
        todo!()
    }

    fn change_epoch(&self, _sender: TransactionSender) -> TransactionResponse {
        todo!()
    }

    fn add_service(
        &self,
        _sender: TransactionSender,
        _service: Service,
        _service_id: ServiceId,
    ) -> TransactionResponse {
        todo!()
    }

    fn remove_service(
        &self,
        _sender: TransactionSender,
        _service_id: ServiceId,
    ) -> TransactionResponse {
        todo!()
    }

    fn slash(
        &self,
        _sender: TransactionSender,
        _proof: ProofOfMisbehavior,
        _service_id: ServiceId,
        _node: NodePublicKey,
    ) -> TransactionResponse {
        todo!()
    }

    /*******External View Functions****** */
    // The following functions should be called through execute_txn as the result of a txn
    // They will never change state even if called through update
    // Will usually only be called through query calls where msg.sender is not checked
    //      so if that is required for the function it should be made a parameter instead

    fn get_flk(&self, _account: AccountOwnerPublicKey) -> TransactionResponse {
        todo!()
    }
    fn get_locked(&self, _account: AccountOwnerPublicKey) -> TransactionResponse {
        todo!()
    }
    fn get_bandwidth(&self, _account: AccountOwnerPublicKey) -> TransactionResponse {
        todo!()
    }
    fn get_staked(&self, _node: NodePublicKey) -> TransactionResponse {
        todo!()
    }
    fn get_reward_pool(&self, _epoch: Epoch) -> TransactionResponse {
        todo!()
    }
    fn get_total_served(&self, _epoch: Epoch) -> TransactionResponse {
        todo!()
    }
    fn get_node_bandwidth_served(
        &self,
        _epoch: Epoch,
        _node: NodePublicKey,
    ) -> TransactionResponse {
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
    fn _distribute_rewards(&self) {
        todo!()
    }
    fn _choose_new_committee(&self) -> Vec<NodePublicKey> {
        todo!()
    }
}
