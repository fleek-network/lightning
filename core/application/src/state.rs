use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::net::IpAddr;
use std::ops::DerefMut;
use std::time::Duration;

use ethers::abi::AbiDecode;
use ethers::types::{Transaction as EthersTransaction, H160};
use fleek_blake3::Hasher;
use fleek_crypto::{
    ClientPublicKey,
    ConsensusPublicKey,
    EthAddress,
    NodePublicKey,
    TransactionSender,
};
use hp_fixed::unsigned::HpUfixed;
use lazy_static::lazy_static;
use lightning_interfaces::types::{
    AccountInfo,
    Blake3Hash,
    Committee,
    CommodityTypes,
    ContentUpdate,
    DeliveryAcknowledgment,
    Epoch,
    ExecutionData,
    ExecutionError,
    Metadata,
    NodeIndex,
    NodeInfo,
    NodePorts,
    NodeServed,
    Participation,
    ProofOfConsensus,
    ProofOfMisbehavior,
    ProtocolParams,
    ReportedReputationMeasurements,
    ReputationMeasurements,
    Service,
    ServiceId,
    ServiceRevenue,
    Staking,
    Tokens,
    TotalServed,
    TransactionRequest,
    TransactionResponse,
    TxHash,
    UpdateMethod,
    UpdateRequest,
    Value,
    MAX_MEASUREMENTS_PER_TX,
    MAX_MEASUREMENTS_SUBMIT,
};
use lightning_interfaces::ToDigest;
use lightning_reputation::statistics;
use lightning_reputation::types::WeightedReputationMeasurements;
use lightning_utils::eth::fleek_contract::FleekContractCalls;
use lightning_utils::eth::{
    DepositCall,
    StakeCall,
    UnstakeCall,
    WithdrawCall,
    WithdrawUnstakedCall,
};
use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::SeedableRng;

use crate::table::{Backend, TableRef};

/// Minimum number of reported measurements that have to be available for a node.
/// If less measurements have been reported, no reputation score will be computed in that epoch.
#[cfg(all(not(test), not(debug_assertions)))]
const MIN_NUM_MEASUREMENTS: usize = 10;
#[cfg(any(test, debug_assertions))]
const MIN_NUM_MEASUREMENTS: usize = 2;

/// Reported measurements are weighted by the reputation score of the reporting node.
/// If there is no reputation score for the reporting node, we use a quantile from the array
/// of all reputation scores.
/// For example, if `DEFAULT_REP_QUANTILE = 0.15`, then we use the reputation score that is higher
/// than 15% of scores and lower than 75% of scores.
const DEFAULT_REP_QUANTILE: f64 = 0.15;

/// The rep score of a node is the exponentially weighted moving average of its rep scores over the
/// past epochs.
/// For example, `REP_EWMA_WEIGHT=0.7` means that 70% of the current rep score is based on past
/// epochs and 30% is based on the current epoch.
const REP_EWMA_WEIGHT: f64 = 0.7;

/// If a node responded to less than 10% of pings from its peers, it will set to inactive until it
/// submits an OptIn transaction.
const MINIMUM_UPTIME: u8 = 40;

/// To support ethereum tooling, all signed ethereum transactions will be pointed to this address
/// otherwise, if there is a value and a different address they are trying to transfer the native
/// token FLK
const FLEEK_CONTRACT: H160 = H160([6; 20]);

/// Max number of updates allowed in a content registry update transaction.
const MAX_UPDATES_CONTENT_REGISTRY: usize = 100;

lazy_static! {
    static ref BIG_HUNDRED: HpUfixed<18> = HpUfixed::<18>::from(100_u64);
}

/// The state of the Application
///
/// The functions implemented on this struct are the "Smart Contracts" of the application layer
/// All state changes come from Transactions and start at execute_transaction
pub struct State<B: Backend> {
    pub metadata: B::Ref<Metadata, Value>,
    pub account_info: B::Ref<EthAddress, AccountInfo>,
    pub client_keys: B::Ref<ClientPublicKey, EthAddress>,
    pub node_info: B::Ref<NodeIndex, NodeInfo>,
    pub consensus_key_to_index: B::Ref<ConsensusPublicKey, NodeIndex>,
    pub pub_key_to_index: B::Ref<NodePublicKey, NodeIndex>,
    pub latencies: B::Ref<(NodeIndex, NodeIndex), Duration>,
    pub committee_info: B::Ref<Epoch, Committee>,
    pub services: B::Ref<ServiceId, Service>,
    pub parameters: B::Ref<ProtocolParams, u128>,
    pub rep_measurements: B::Ref<NodeIndex, Vec<ReportedReputationMeasurements>>,
    pub rep_scores: B::Ref<NodeIndex, u8>,
    pub submitted_rep_measurements: B::Ref<NodeIndex, u8>,
    pub current_epoch_served: B::Ref<NodeIndex, NodeServed>,
    pub last_epoch_served: B::Ref<NodeIndex, NodeServed>,
    pub total_served: B::Ref<Epoch, TotalServed>,
    pub service_revenue: B::Ref<ServiceId, ServiceRevenue>,
    pub commodity_prices: B::Ref<CommodityTypes, HpUfixed<6>>,
    pub executed_digests: B::Ref<TxHash, ()>,
    pub uptime: B::Ref<NodeIndex, u8>,
    pub cid_to_node: B::Ref<Blake3Hash, BTreeSet<NodeIndex>>,
    pub node_to_cid: B::Ref<NodeIndex, BTreeSet<Blake3Hash>>,
    pub backend: B,
}

impl<B: Backend> State<B> {
    pub fn new(backend: B) -> Self {
        Self {
            metadata: backend.get_table_reference("metadata"),
            account_info: backend.get_table_reference("account"),
            client_keys: backend.get_table_reference("client_keys"),
            node_info: backend.get_table_reference("node"),
            consensus_key_to_index: backend.get_table_reference("consensus_key_to_index"),
            pub_key_to_index: backend.get_table_reference("pub_key_to_index"),
            committee_info: backend.get_table_reference("committee"),
            services: backend.get_table_reference("service"),
            parameters: backend.get_table_reference("parameter"),
            rep_measurements: backend.get_table_reference("rep_measurements"),
            latencies: backend.get_table_reference("latencies"),
            rep_scores: backend.get_table_reference("rep_scores"),
            submitted_rep_measurements: backend.get_table_reference("submitted_rep_measurements"),
            last_epoch_served: backend.get_table_reference("last_epoch_served"),
            current_epoch_served: backend.get_table_reference("current_epoch_served"),
            total_served: backend.get_table_reference("total_served"),
            commodity_prices: backend.get_table_reference("commodity_prices"),
            service_revenue: backend.get_table_reference("service_revenue"),
            executed_digests: backend.get_table_reference("executed_digests"),
            uptime: backend.get_table_reference("uptime"),
            cid_to_node: backend.get_table_reference("cid_to_node"),
            node_to_cid: backend.get_table_reference("node_to_cid"),
            backend,
        }
    }

    pub fn execute_transaction(&self, txn: TransactionRequest) -> TransactionResponse {
        let hash = txn.hash();
        let (sender, response) = match txn {
            TransactionRequest::UpdateRequest(payload) => (
                payload.payload.sender,
                self.execute_fleek_transaction(payload),
            ),
            TransactionRequest::EthereumRequest(payload) => (
                TransactionSender::AccountOwner(EthAddress(payload.from.0)),
                self.execute_ethereum_transaction(payload.into()),
            ),
        };
        self.executed_digests.set(hash, ());
        // Increment nonce of the sender
        self.increment_nonce(sender);
        response
    }

    /// This function is the entry point of a transaction
    fn execute_fleek_transaction(&self, txn: UpdateRequest) -> TransactionResponse {
        // Execute transaction
        let response = match txn.payload.method {
            UpdateMethod::SubmitDeliveryAcknowledgmentAggregation {
                commodity,
                service_id,
                proofs,
                metadata: _,
            } => self.submit_pod(txn.payload.sender, commodity, service_id, proofs),

            UpdateMethod::Withdraw {
                amount,
                token,
                receiving_address,
            } => self.withdraw(txn.payload.sender, receiving_address, amount, token),

            UpdateMethod::Deposit {
                proof,
                token,
                amount,
            } => self.deposit(txn.payload.sender, proof, amount, token),

            UpdateMethod::Transfer { amount, token, to } => {
                self.transfer(txn.payload.sender, amount, token, to)
            },

            UpdateMethod::Stake {
                amount,
                node_public_key,
                consensus_key,
                node_domain,
                worker_public_key,
                worker_domain,
                ports,
            } => self.stake(
                txn.payload.sender,
                amount,
                node_public_key,
                consensus_key,
                node_domain,
                worker_public_key,
                worker_domain,
                ports,
            ),
            UpdateMethod::StakeLock { node, locked_for } => {
                self.stake_lock(txn.payload.sender, node, locked_for)
            },

            UpdateMethod::Unstake { amount, node } => {
                self.unstake(txn.payload.sender, amount, node)
            },

            UpdateMethod::WithdrawUnstaked { node, recipient } => {
                self.withdraw_unstaked(txn.payload.sender, node, recipient)
            },

            UpdateMethod::ChangeEpoch { epoch } => self.change_epoch(txn.payload.sender, epoch),

            UpdateMethod::AddService {
                service,
                service_id,
            } => self.add_service(txn.payload.sender, service, service_id),

            UpdateMethod::RemoveService { service_id } => {
                self.remove_service(txn.payload.sender, service_id)
            },

            UpdateMethod::Slash {
                service_id,
                node,
                proof_of_misbehavior,
            } => self.slash(txn.payload.sender, proof_of_misbehavior, service_id, node),

            UpdateMethod::SubmitReputationMeasurements { measurements } => {
                self.submit_reputation_measurements(txn.payload.sender, measurements)
            },
            UpdateMethod::ChangeProtocolParam { param, value } => {
                self.change_protocol_param(txn.payload.sender, param, value)
            },
            UpdateMethod::OptIn {} => self.opt_in(txn.payload.sender),
            UpdateMethod::OptOut {} => self.opt_out(txn.payload.sender),
            UpdateMethod::UpdateContentRegistry { updates } => {
                self.update_content_registry(txn.payload.sender, updates)
            },
        };

        #[cfg(debug_assertions)]
        {
            let node_info_len = self.node_info.keys().count();
            let consensus_key_to_index_len = self.consensus_key_to_index.keys().count();
            let pub_key_to_index_len = self.pub_key_to_index.keys().count();
            assert_eq!(node_info_len, consensus_key_to_index_len);
            assert_eq!(node_info_len, pub_key_to_index_len);
            assert_eq!(pub_key_to_index_len, consensus_key_to_index_len);
        }

        response
    }

    fn execute_ethereum_transaction(&self, txn: EthersTransaction) -> TransactionResponse {
        let to_address = match txn.to {
            Some(address) => address,
            // Either the transaction is going to the "Fleek Contract" for a state transition or its
            // to another address for a transfer
            None => return TransactionResponse::Revert(ExecutionError::InvalidStateFunction),
        };
        let sender: EthAddress = txn.from.0.into();

        if to_address == FLEEK_CONTRACT {
            // They are calling one of our state transitions functions
            #[allow(unused)]
            match FleekContractCalls::decode(&txn.input) {
                Ok(FleekContractCalls::Deposit(DepositCall { token, amount })) => {
                    // TODO(matthias): add proof of consensus to abi once we know its format
                    let Ok(token) = Tokens::try_from(token) else {
                        return TransactionResponse::Revert(ExecutionError::InvalidToken);
                    };
                    self.deposit(sender.into(), ProofOfConsensus {}, amount.into(), token)
                },
                Ok(FleekContractCalls::Withdraw(WithdrawCall {
                    amount,
                    token,
                    recipient,
                })) => {
                    let Ok(token) = Tokens::try_from(token) else {
                        return TransactionResponse::Revert(ExecutionError::InvalidToken);
                    };
                    self.withdraw(sender.into(), recipient.0.into(), amount.into(), token)
                },
                Ok(FleekContractCalls::Unstake(UnstakeCall {
                    amount,
                    node_public_key,
                })) => self.unstake(sender.into(), amount.into(), node_public_key.into()),
                Ok(FleekContractCalls::WithdrawUnstaked(WithdrawUnstakedCall {
                    node_public_key,
                    recipient,
                })) => self.withdraw_unstaked(sender.into(), node_public_key.into(), None),
                Ok(FleekContractCalls::Stake(StakeCall {
                    amount,
                    node_public_key,
                    consensus_key,
                    domain,
                    worker_public_key,
                    worker_domain,
                })) => {
                    let Ok(node_domain) = domain.parse() else {
                        return TransactionResponse::Revert(ExecutionError::InvalidInternetAddress);
                    };
                    let consensus_key_bytes: [u8; 96] = match consensus_key.to_vec().try_into() {
                        Ok(bytes) => bytes,
                        Err(_) => {
                            return TransactionResponse::Revert(
                                ExecutionError::InvalidConsensusKey,
                            );
                        },
                    };
                    let consensus_key: ConsensusPublicKey = consensus_key_bytes.into();
                    let node_public_key: NodePublicKey = node_public_key.into();

                    self.stake(
                        sender.into(),
                        amount.into(),
                        node_public_key,
                        Some(consensus_key),
                        Some(node_domain),
                        Some(node_public_key),
                        Some(node_domain),
                        // TODO(matthias): allow senders to specify ports?
                        Some(NodePorts::default()),
                    )
                },
                _ => TransactionResponse::Revert(ExecutionError::InvalidStateFunction),
            }
        } else {
            // They are trying to transfer FLK
            self.transfer(
                sender.into(),
                txn.value.into(),
                Tokens::FLK,
                to_address.0.into(),
            )
        }
    }

    /*********** External Update Functions ********** */
    // The following functions should only be called in the result of a query or update transaction
    // through execute_txn() If called in an update txn it will mutate state
    fn submit_pod(
        &self,
        sender: TransactionSender,
        commodity: u128,
        service_id: u32,
        _acknowledgments: Vec<DeliveryAcknowledgment>,
    ) -> TransactionResponse {
        // Todo: function not done
        let sender: NodeIndex = match self.only_node(sender) {
            Ok(index) => index,
            Err(e) => return e,
        };
        let account_owner = match self.node_info.get(&sender) {
            Some(node_info) => node_info.owner,
            None => return TransactionResponse::Revert(ExecutionError::NodeDoesNotExist),
        };

        if self.services.get(&service_id).is_none() {
            return TransactionResponse::Revert(ExecutionError::InvalidServiceId);
        }
        // Todo: build proof based on delivery acks
        if !self.verify_proof_of_delivery(&account_owner, &sender, &commodity, &service_id, ()) {
            return TransactionResponse::Revert(ExecutionError::InvalidProof);
        }

        let commodity_type = self
            .services
            .get(&service_id)
            .map(|s| s.commodity_type)
            .unwrap();

        let current_epoch = match self.metadata.get(&Metadata::Epoch) {
            Some(Value::Epoch(epoch)) => epoch,
            _ => 0,
        };

        let mut node_served = self.current_epoch_served.get(&sender).unwrap_or_default();
        let mut total_served = self.total_served.get(&current_epoch).unwrap_or_default();
        let commodity_prices = self
            .commodity_prices
            .get(&commodity_type)
            .expect("Commodity price should always be set");

        let commodity_index = commodity_type as usize;
        for i in 0..=commodity_index {
            if i >= node_served.served.len() {
                node_served.served.push(0);
            }
            if i >= total_served.served.len() {
                total_served.served.push(0);
            }
        }
        let commodity_to_big: HpUfixed<6> = commodity.into();
        let revenue = &commodity_to_big * &commodity_prices;

        node_served.served[commodity_index] += commodity;
        node_served.stables_revenue += revenue.clone();

        total_served.served[commodity_index] += commodity;
        total_served.reward_pool += revenue.clone();

        // Todo: track commodity served by service to be used for service builders reward share
        // if the a services serves multiple commodity, the current logic would change
        let mut service_revenue = self.service_revenue.get(&service_id).unwrap_or_default();
        service_revenue += revenue;

        self.current_epoch_served.set(sender, node_served);
        self.total_served.set(current_epoch, total_served);
        self.service_revenue.set(service_id, service_revenue);

        TransactionResponse::Success(ExecutionData::None)
    }

    fn withdraw(
        &self,
        _sender: TransactionSender,
        _reciever: EthAddress,
        _amount: HpUfixed<18>,
        _token: Tokens,
    ) -> TransactionResponse {
        TransactionResponse::Revert(ExecutionError::Unimplemented)
    }

    fn deposit(
        &self,
        sender: TransactionSender,
        proof: ProofOfConsensus,
        amount: HpUfixed<18>,
        token: Tokens,
    ) -> TransactionResponse {
        // This transaction is only callable by AccountOwners and not nodes
        // So revert if the sender is a node public key
        let sender = match self.only_account_owner(sender) {
            Ok(account) => account,
            Err(e) => return e,
        };

        // Verify the proof from the bridge
        if !self.verify_proof_of_consensus(proof) {
            return TransactionResponse::Revert(ExecutionError::InvalidProof);
        }

        let mut account = self.account_info.get(&sender).unwrap_or_default();

        // Check the token bridged and increment that amount
        match token {
            Tokens::FLK => account.flk_balance += amount,
            Tokens::USDC => account.bandwidth_balance += TryInto::<u128>::try_into(amount).unwrap(),
        }

        self.account_info.set(sender, account);
        TransactionResponse::Success(ExecutionData::None)
    }

    fn transfer(
        &self,
        sender: TransactionSender,
        amount: HpUfixed<18>,
        _token: Tokens,
        to: EthAddress,
    ) -> TransactionResponse {
        // Todo(Dalton): Figure out if we should support transferring other tokens like usdc within
        // our network for now im going to treat this function as just transferring FLK

        // This transaction is only callable by AccountOwners and not nodes
        // So revert if the sender is a node public key
        let sender = match self.only_account_owner(sender) {
            Ok(account) => account,
            Err(e) => return e,
        };
        let mut sender_account = self.account_info.get(&sender).unwrap_or_default();
        let mut to_account = self.account_info.get(&to).unwrap_or_default();

        if sender_account == to_account {
            return TransactionResponse::Revert(ExecutionError::CantSendToYourself);
        }
        // Check that they have the funds
        if sender_account.flk_balance < amount {
            return TransactionResponse::Revert(ExecutionError::InsufficientBalance);
        }
        // todo(dal)
        sender_account.flk_balance -= amount.clone();

        // Increment the to address balance
        to_account.flk_balance += amount;

        // set both fields in our tables
        self.account_info.set(to, to_account);
        self.account_info.set(sender, sender_account);

        TransactionResponse::Success(ExecutionData::None)
    }

    #[allow(clippy::too_many_arguments)]
    fn stake(
        &self,
        sender: TransactionSender,
        amount: HpUfixed<18>,
        node_public_key: NodePublicKey,
        node_consensus_key: Option<ConsensusPublicKey>,
        node_domain: Option<IpAddr>,
        worker_public_key: Option<NodePublicKey>,
        worker_domain: Option<IpAddr>,
        ports: Option<NodePorts>,
    ) -> TransactionResponse {
        // This transaction is only callable by AccountOwners and not nodes
        // So revert if the sender is a node public key
        let sender = match self.only_account_owner(sender) {
            Ok(account) => account,
            Err(e) => return e,
        };

        let current_epoch = match self.metadata.get(&Metadata::Epoch) {
            Some(Value::Epoch(epoch)) => epoch,
            _ => 0,
        };

        let mut owner = self.account_info.get(&sender).unwrap_or_default();

        // Make sure the sender has at least the amount of FLK he is trying to stake
        if owner.flk_balance < amount {
            return TransactionResponse::Revert(ExecutionError::InsufficientBalance);
        }

        let node_index = self.pub_key_to_index.get(&node_public_key);
        // Make sure the networking index and bls are the same
        if let Some(consensus_key) = node_consensus_key {
            // If the consensus key is indexed make sure it is the same as the indexed node public
            // key, if its none indexed and the node key is, that is fine
            if self.consensus_key_to_index.get(&consensus_key) != node_index {
                return TransactionResponse::Revert(ExecutionError::ConsensusKeyAlreadyIndexed);
            }
        }

        match node_index {
            Some(index) => {
                let mut node = match self.node_info.get(&index) {
                    Some(node) => node,
                    // This should be impossible to hit
                    None => return TransactionResponse::Revert(ExecutionError::NodeDoesNotExist),
                };
                // Todo(dalton): should we stop people from staking on a node they do not own??

                if let Some(primary_domain) = node_domain {
                    node.domain = primary_domain;
                }
                if let Some(worker_key) = worker_public_key {
                    node.worker_public_key = worker_key;
                }
                if let Some(worker_domain) = worker_domain {
                    node.worker_domain = worker_domain;
                }
                if let Some(port) = ports {
                    node.ports = port;
                }

                // Increase the nodes stake by the amount being staked
                node.stake.staked += amount.clone();
                self.node_info.set(index, node);
            },
            None => {
                // If the node doesn't Exist, create it. But check if they provided all the required
                // options for a new node
                if let (
                    Some(consensus_key),
                    Some(domain),
                    Some(worker_public_key),
                    Some(worker_domain),
                    Some(ports),
                ) = (
                    node_consensus_key,
                    node_domain,
                    worker_public_key,
                    worker_domain,
                    ports,
                ) {
                    let node = NodeInfo {
                        owner: sender,
                        public_key: node_public_key,
                        consensus_key,
                        worker_public_key,
                        staked_since: current_epoch,
                        stake: Staking {
                            staked: amount.clone(),
                            ..Default::default()
                        },
                        domain,
                        worker_domain,
                        ports,
                        participation: Participation::False,
                        nonce: 0,
                    };
                    self.create_node(node);
                } else {
                    return TransactionResponse::Revert(ExecutionError::InsufficientNodeDetails);
                }
            },
        }

        // decrement the owners balance
        owner.flk_balance -= amount;

        // Commit changes to the owner
        self.account_info.set(sender, owner);
        TransactionResponse::Success(ExecutionData::None)
    }

    fn stake_lock(
        &self,
        sender: TransactionSender,
        node_public_key: NodePublicKey,
        locked_for: u64,
    ) -> TransactionResponse {
        // This transaction is only callable by AccountOwners and not nodes
        // So revert if the sender is a node public key
        let sender = match self.only_account_owner(sender) {
            Ok(account) => account,
            Err(e) => return e,
        };

        let (index, mut node) = match self.get_node_info(node_public_key.into()) {
            Some(node) => node,
            None => return TransactionResponse::Revert(ExecutionError::NodeDoesNotExist),
        };

        // Make sure the caller is the owner of the node
        if sender != node.owner {
            return TransactionResponse::Revert(ExecutionError::NotNodeOwner);
        }
        // check if node has stakes to be locked
        if node.stake.staked == HpUfixed::zero() {
            return TransactionResponse::Revert(ExecutionError::InsufficientStake);
        }

        let epoch = match self.metadata.get(&Metadata::Epoch) {
            Some(Value::Epoch(epoch)) => epoch,
            _ => 0,
        };
        let current_lock = node.stake.stake_locked_until.saturating_sub(epoch);

        let max_lock_time = self
            .parameters
            .get(&ProtocolParams::MaxStakeLockTime)
            .unwrap_or(1);

        // check if total locking is greater than max locking
        if current_lock + locked_for > max_lock_time as u64 {
            return TransactionResponse::Revert(ExecutionError::LockExceededMaxStakeLockTime);
        }

        node.stake.stake_locked_until += locked_for;

        // Save the changed node state and return success
        self.node_info.set(index, node);
        TransactionResponse::Success(ExecutionData::None)
    }

    fn unstake(
        &self,
        sender: TransactionSender,
        amount: HpUfixed<18>,
        node_public_key: NodePublicKey,
    ) -> TransactionResponse {
        // This transaction is only callable by AccountOwners and not nodes
        // So revert if the sender is a node public key
        let sender = match self.only_account_owner(sender) {
            Ok(account) => account,
            Err(e) => return e,
        };

        let (index, mut node) = match self.get_node_info(node_public_key.into()) {
            Some(node) => node,
            None => return TransactionResponse::Revert(ExecutionError::NodeDoesNotExist),
        };

        // Make sure the caller is the owner of the node
        if sender != node.owner {
            return TransactionResponse::Revert(ExecutionError::NotNodeOwner);
        }

        let current_epoch = match self.metadata.get(&Metadata::Epoch) {
            Some(Value::Epoch(epoch)) => epoch,
            _ => 0,
        };

        // Make sure the stakes are not locked
        if node.stake.stake_locked_until > current_epoch {
            return TransactionResponse::Revert(ExecutionError::LockedTokensUnstakeForbidden);
        }

        // Make sure the node has at least that much staked
        if node.stake.staked < amount {
            return TransactionResponse::Revert(ExecutionError::InsufficientBalance);
        }

        let lock_time = self.parameters.get(&ProtocolParams::LockTime).unwrap_or(0);

        // decrease the stake, add to the locked amount, and set the locked time for the withdrawal
        // current epoch + lock time todo(dalton): we should be storing unstaked tokens in a
        // list so we can have multiple locked stakes with dif lock times
        node.stake.staked -= amount.clone();
        node.stake.locked += amount;
        node.stake.locked_until = current_epoch + lock_time as u64;

        // Save the changed node state and return success
        self.node_info.set(index, node);
        TransactionResponse::Success(ExecutionData::None)
    }

    fn withdraw_unstaked(
        &self,
        sender: TransactionSender,
        node_public_key: NodePublicKey,
        recipient: Option<EthAddress>,
    ) -> TransactionResponse {
        // This transaction is only callable by AccountOwners and not nodes
        // So revert if the sender is a node public key
        let sender_public_key = match self.only_account_owner(sender) {
            Ok(account) => account,
            Err(e) => return e,
        };

        let (index, mut node) = match self.get_node_info(node_public_key.into()) {
            Some(node) => node,
            None => return TransactionResponse::Revert(ExecutionError::NodeDoesNotExist),
        };

        // Make sure the caller is the owner of the node
        if sender_public_key != node.owner {
            return TransactionResponse::Revert(ExecutionError::NotNodeOwner);
        }

        let current_epoch = match self.metadata.get(&Metadata::Epoch) {
            Some(Value::Epoch(epoch)) => epoch,
            _ => 0,
        };
        // Make sure the node has locked tokens and that the lock time is passed
        if node.stake.locked == HpUfixed::zero() {
            return TransactionResponse::Revert(ExecutionError::NoLockedTokens);
        }
        if node.stake.locked_until > current_epoch {
            return TransactionResponse::Revert(ExecutionError::TokensLocked);
        }

        // if there is no recipient the owner will receive the withdrawal
        let recipient = recipient.unwrap_or(sender_public_key);
        let mut receiver = self.account_info.get(&recipient).unwrap_or_default();

        // add the withdrawn tokens to the recipient and reset the nodes locked stake state
        // no need to reset locked_until on the node because that will get adjusted the next time
        // the node unstakes
        receiver.flk_balance += node.stake.locked;
        node.stake.locked = HpUfixed::zero();

        // Todo(dalton): if the nodes stake+locked are equal to 0 here should we remove him from the
        // state tables completely?

        // Save state changes and return response
        self.account_info.set(recipient, receiver);
        self.node_info.set(index, node);
        TransactionResponse::Success(ExecutionData::None)
    }

    fn change_epoch(&self, sender: TransactionSender, epoch: Epoch) -> TransactionResponse {
        // Only Nodes can call this function
        let index = match self.only_node(sender) {
            Ok(account) => account,
            Err(e) => return e,
        };
        let mut current_epoch = match self.metadata.get(&Metadata::Epoch) {
            Some(Value::Epoch(epoch)) => epoch,
            _ => 0,
        };

        match epoch.cmp(&current_epoch) {
            Ordering::Less => {
                return TransactionResponse::Revert(ExecutionError::EpochAlreadyChanged);
            },
            Ordering::Greater => {
                return TransactionResponse::Revert(ExecutionError::EpochHasNotStarted);
            },
            _ => (),
        }

        let mut current_committee = self.committee_info.get(&current_epoch).unwrap_or_default();

        // If sender is not on the current committee revert early, or if they have already signaled;
        if !current_committee.members.contains(&index) {
            return TransactionResponse::Revert(ExecutionError::NotCommitteeMember);
        } else if current_committee.ready_to_change.contains(&index) {
            return TransactionResponse::Revert(ExecutionError::AlreadySignaled);
        }
        current_committee.ready_to_change.push(index);

        // If more than 2/3rds of the committee have signaled, start the epoch change process
        if current_committee.ready_to_change.len() > (2 * current_committee.members.len() / 3) {
            // Todo: Reward nodes, choose new committee, increment epoch.
            self.calculate_reputation_scores();
            self.distribute_rewards();
            // Todo: We can't really fail after here
            // because changes have already been submitted above
            // in the call to calculate_reputation_scores.
            // Should we refactor change_epoch so it operates in two steps?
            //  1. Validate all mutations that will be made and stage them.
            //  2. Submit staged changes.
            // Then, `clear_content_registry` could become
            // `stage_clear_content_registry' and return the new state for the
            // tables instead of applying the changes itself.
            self.clean_up_content_registry();

            // Clear executed digests.
            for digest in self.executed_digests.keys() {
                self.executed_digests.remove(&digest);
            }

            // calculate the next epoch endstamp
            let epoch_duration = self.parameters.get(&ProtocolParams::EpochTime).unwrap_or(1);

            let new_epoch_end = current_committee.epoch_end_timestamp + epoch_duration as u64;
            // Save the old committee so we can see who signaled
            self.committee_info.set(current_epoch, current_committee);
            // Get new committee
            let new_committee = self.choose_new_committee();
            // increment epoch
            current_epoch += 1;

            self.committee_info.set(
                current_epoch,
                Committee {
                    ready_to_change: Vec::with_capacity(new_committee.len()),
                    members: new_committee,
                    epoch_end_timestamp: new_epoch_end,
                },
            );

            self.metadata
                .set(Metadata::Epoch, Value::Epoch(current_epoch));
            TransactionResponse::Success(ExecutionData::EpochChange)
        } else {
            self.committee_info.set(current_epoch, current_committee);
            TransactionResponse::Success(ExecutionData::None)
        }
    }

    fn calculate_reputation_scores(&self) {
        let mut rep_scores = HashMap::new();
        self.rep_scores.keys().for_each(|node| {
            if let Some(score) = self.rep_scores.get(&node) {
                rep_scores.insert(node, score);
            }
        });
        let default_score = statistics::approx_quantile(
            rep_scores.values().copied().collect(),
            HpUfixed::<18>::from(DEFAULT_REP_QUANTILE),
        )
        .unwrap_or(15);

        let mut map = HashMap::new();
        for node in self.rep_measurements.keys() {
            if let Some(reported_measurements) = self.rep_measurements.get(&node) {
                if reported_measurements.len() >= MIN_NUM_MEASUREMENTS {
                    // Only compute reputation score for node if enough measurements have been
                    // reported
                    let weighted_measurements = reported_measurements
                        .into_iter()
                        .map(|m| {
                            let weight = self
                                .rep_scores
                                .get(&m.reporting_node)
                                .unwrap_or(default_score);
                            WeightedReputationMeasurements {
                                measurements: m.measurements,
                                weight,
                            }
                        })
                        .collect();
                    map.insert(node, weighted_measurements);
                }
            }
        }
        // Clear the uptime measurements from the previous epoch.
        let nodes = self.uptime.keys();
        nodes.for_each(|node| self.uptime.remove(&node));

        // Store new scores in application state.
        let new_rep_scores = lightning_reputation::calculate_reputation_scores(map);
        let nodes = self.node_info.keys();
        nodes.for_each(|node| {
            let (new_score, uptime) = match new_rep_scores.get(&node) {
                Some((new_score, uptime)) => (*new_score, *uptime),
                None => (None, None),
            };

            let old_score = rep_scores.get(&node).unwrap_or(&default_score);
            let new_score = new_score.unwrap_or(0);
            let emwa_weight = HpUfixed::<18>::from(REP_EWMA_WEIGHT);
            let score = HpUfixed::<18>::from(*old_score as u32) * emwa_weight.clone()
                + (HpUfixed::<18>::from(1.0) - emwa_weight)
                    * HpUfixed::<18>::from(new_score as u32);
            let score: u128 = score.try_into().unwrap_or(default_score as u128);
            // The value of score will be in range [0, 100]
            self.rep_scores.set(node, score as u8);

            let mut node_info = self.node_info.get(&node).unwrap();
            if node_info.participation == Participation::OptedIn {
                node_info.participation = Participation::True;
            }
            if node_info.participation == Participation::OptedOut {
                node_info.participation = Participation::False;
            }
            self.node_info.set(node, node_info);

            if let Some(uptime) = uptime {
                self.uptime.set(node, uptime);
                if uptime < MINIMUM_UPTIME {
                    if let Some(mut node_info) = self.node_info.get(&node) {
                        node_info.participation = Participation::False;
                        self.node_info.set(node, node_info);
                    }
                }
            }
        });

        self.update_latencies();

        // Remove measurements from this epoch once we calculated the rep scores.
        let nodes = self.rep_measurements.keys();
        nodes.for_each(|node| self.rep_measurements.remove(&node));

        // Reset the already submitted flags so that nodes can submit measurements again in the new
        // epoch.
        let nodes = self.submitted_rep_measurements.keys();
        nodes.for_each(|node| self.submitted_rep_measurements.remove(&node));
    }

    fn update_latencies(&self) {
        // Remove latency measurements from invalid nodes.
        let node_registry = self.get_node_registry();
        for (index_lhs, index_rhs) in self.latencies.keys() {
            if !node_registry.contains_key(&index_lhs) || !node_registry.contains_key(&index_rhs) {
                self.latencies.remove(&(index_lhs, index_rhs));
            }
        }

        // Process latency measurements. If latency measurements are available for both directions
        // between two nodes, we use the average.
        let mut latency_map = HashMap::new();
        for node in self.rep_measurements.keys() {
            if let Some(reported_measurements) = self.rep_measurements.get(&node) {
                for measurement in reported_measurements {
                    if let Some(latency) = measurement.measurements.latency {
                        let (node_lhs, node_rhs) = if node < measurement.reporting_node {
                            (node, measurement.reporting_node)
                        } else {
                            (measurement.reporting_node, node)
                        };
                        let latency =
                            if let Some(opp_latency) = latency_map.get(&(node_lhs, node_rhs)) {
                                (latency + *opp_latency) / 2
                            } else {
                                latency
                            };
                        latency_map.insert((node_lhs, node_rhs), latency);
                    }
                }
            }
        }

        // Store the latencies that were reported in this epoch.
        // This will potentially overwrite latency measurements from previous epochs.
        for ((index_lhs, index_rhs), latency) in latency_map {
            // Todo (dalton): Check if this check is needed, it may be done before being added to
            // latancy map
            if self.node_info.get(&index_lhs).is_some() && self.node_info.get(&index_rhs).is_some()
            {
                self.latencies.set((index_lhs, index_rhs), latency);
            }
        }
    }

    // This method can panic if the governance address wasn't previously stored in the application
    // state. The governance address should be seeded though the genesis.
    fn change_protocol_param(
        &self,
        sender: TransactionSender,
        param: ProtocolParams,
        value: u128,
    ) -> TransactionResponse {
        let sender = match self.only_account_owner(sender) {
            Ok(account) => account,
            Err(e) => return e,
        };
        // TODO(matthias): should be panic here or revert? Since the governance address will be
        // seeded though genesis, this should never happen.
        let governance_address = match self.metadata.get(&Metadata::GovernanceAddress) {
            Some(Value::AccountPublicKey(address)) => address,
            _ => panic!("Governance address is missing from state."),
        };
        if sender != governance_address {
            return TransactionResponse::Revert(ExecutionError::OnlyGovernance);
        }
        self.parameters.set(param, value);
        TransactionResponse::Success(ExecutionData::None)
    }

    fn opt_in(&self, sender: TransactionSender) -> TransactionResponse {
        let index = match self.only_node(sender) {
            Ok(account) => account,
            Err(e) => return e,
        };
        match self.node_info.get(&index) {
            Some(mut node_info) => {
                node_info.participation = Participation::OptedIn;
                self.node_info.set(index, node_info);
                TransactionResponse::Success(ExecutionData::None)
            },
            None => TransactionResponse::Revert(ExecutionError::NodeDoesNotExist),
        }
    }

    fn opt_out(&self, sender: TransactionSender) -> TransactionResponse {
        let index = match self.only_node(sender) {
            Ok(account) => account,
            Err(e) => return e,
        };
        match self.node_info.get(&index) {
            Some(mut node_info) => {
                node_info.participation = Participation::OptedOut;
                self.node_info.set(index, node_info);
                TransactionResponse::Success(ExecutionData::None)
            },
            None => TransactionResponse::Revert(ExecutionError::NodeDoesNotExist),
        }
    }

    fn get_node_registry(&self) -> BTreeMap<NodeIndex, NodeInfo> {
        let minimum_stake = self
            .parameters
            .get(&ProtocolParams::MinimumNodeStake)
            .unwrap_or(0);
        self.node_info
            .keys()
            .filter_map(|key| self.node_info.get(&key).map(|node| (key, node)))
            .filter(|(_, node)| node.stake.staked >= minimum_stake.into())
            .collect()
    }

    // This function should only be called in the `run` method on `Env`.
    pub fn set_last_epoch_hash(&self, state_hash: [u8; 32]) {
        self.metadata
            .set(Metadata::LastEpochHash, Value::Hash(state_hash));
    }

    // This function should only be called in the `run` method on `Env`.
    pub fn set_last_block(&self, block_hash: [u8; 32]) {
        self.metadata
            .set(Metadata::LastBlockHash, Value::Hash(block_hash));
    }

    fn add_service(
        &self,
        _sender: TransactionSender,
        _service: Service,
        _service_id: ServiceId,
    ) -> TransactionResponse {
        TransactionResponse::Revert(ExecutionError::Unimplemented)
    }

    fn remove_service(
        &self,
        _sender: TransactionSender,
        _service_id: ServiceId,
    ) -> TransactionResponse {
        TransactionResponse::Revert(ExecutionError::Unimplemented)
    }

    fn slash(
        &self,
        _sender: TransactionSender,
        _proof: ProofOfMisbehavior,
        _service_id: ServiceId,
        _node: NodePublicKey,
    ) -> TransactionResponse {
        TransactionResponse::Revert(ExecutionError::Unimplemented)
    }

    fn submit_reputation_measurements(
        &self,
        sender: TransactionSender,
        measurements: BTreeMap<NodeIndex, ReputationMeasurements>,
    ) -> TransactionResponse {
        let reporting_node = match self.only_node(sender) {
            Ok(index) => index,
            Err(e) => return e,
        };
        let num_submissions = self
            .submitted_rep_measurements
            .get(&reporting_node)
            .unwrap_or(0);
        if num_submissions >= MAX_MEASUREMENTS_SUBMIT {
            return TransactionResponse::Revert(ExecutionError::SubmittedTooManyTransactions);
        }
        // We set this value before the `TooManyMeasurements` revert on purpose.
        self.submitted_rep_measurements
            .set(reporting_node, num_submissions + 1);

        if measurements.len() > MAX_MEASUREMENTS_PER_TX
            || measurements.len() > self.node_info.keys().count()
        {
            return TransactionResponse::Revert(ExecutionError::TooManyMeasurements);
        }

        measurements
            .into_iter()
            .for_each(|(peer_index, measurements)| {
                if peer_index != reporting_node
                    && self.is_valid_node(&peer_index).unwrap_or(false)
                    && measurements.verify()
                {
                    let mut node_measurements = match self.rep_measurements.get(&peer_index) {
                        Some(node_measurements) => node_measurements,
                        None => Vec::new(),
                    };
                    node_measurements.push(ReportedReputationMeasurements {
                        reporting_node,
                        measurements,
                    });
                    self.rep_measurements.set(peer_index, node_measurements);
                }
            });
        TransactionResponse::Success(ExecutionData::None)
    }

    fn update_content_registry(
        &self,
        sender: TransactionSender,
        updates: Vec<ContentUpdate>,
    ) -> TransactionResponse {
        if updates.len() > MAX_UPDATES_CONTENT_REGISTRY {
            return TransactionResponse::Revert(ExecutionError::TooManyUpdates);
        }

        let node_index = match self.only_node(sender) {
            Ok(index) => index,
            Err(e) => return e,
        };

        let mut staged_cid_to_node = HashMap::new();
        let mut staged_cids_provided = self.node_to_cid.get(&node_index).unwrap_or_default();
        let empty_cids_provided = staged_cids_provided.is_empty();

        for update in updates {
            // Check if they sent multiple updates for the same CID.
            if staged_cid_to_node.contains_key(&update.cid) {
                return TransactionResponse::Revert(ExecutionError::TooManyUpdatesForContent);
            }

            let providers = self.cid_to_node.get(&update.cid);

            // Check if a removal request makes sense given our state.
            if update.remove && (providers.is_none() || empty_cids_provided) {
                return TransactionResponse::Revert(ExecutionError::InvalidStateForContentRemoval);
            }

            let mut providers = providers.unwrap_or_default();

            if update.remove {
                if !providers.remove(&node_index) {
                    return TransactionResponse::Revert(ExecutionError::InvalidContentRemoval);
                }
                if !staged_cids_provided.remove(&update.cid) {
                    // Unreachable.
                    return TransactionResponse::Revert(ExecutionError::InvalidContentRemoval);
                }
            } else {
                if !providers.insert(node_index) {
                    return TransactionResponse::Revert(ExecutionError::ContentAlreadyRegistered);
                }
                if !staged_cids_provided.insert(update.cid) {
                    // Unreachable.
                    return TransactionResponse::Revert(ExecutionError::ContentAlreadyRegistered);
                }
            }

            staged_cid_to_node.insert(update.cid, providers);
        }

        self.node_to_cid.set(node_index, staged_cids_provided);

        for (cid, providers) in staged_cid_to_node {
            self.cid_to_node.set(cid, providers);
        }

        TransactionResponse::Success(ExecutionData::None)
    }

    /********Internal Application Functions******** */
    // These functions should only ever be called in the context of an external transaction function
    // They should never panic and any check that could result in that should be done in the
    // external function that calls it The functions that should call this and the required
    // checks should be documented for each function

    /// Distributes rewards among the nodes in a network.
    ///
    /// This function should be invoked during the `signal_epoch_change` to distribute rewards for
    /// the new epoch. It distributes rewards based on the amount of service provided by each node.
    /// It also takes into account the locked stake of each node to provide boosted rewards.
    /// This boost increases the longer the stake is locked. The rewards are distributed based on
    /// emissions per unit revenue to account for inflation and max boost parameters.
    ///
    /// This function calculates the rewards for each node that has served commodities in the
    /// current epoch. The rewards are given in two forms: 1) A `stable` currency that is
    /// proportional to the revenue earned by selling the commodities. 2) `FLK` token that is
    /// proportional to the `stable` currency rewards, but also depends on a `boost` factor.
    ///
    /// `FLk` total emission is given by:
    /// `emission = (inflation * supply) / (daysInYear=365.0)`
    fn distribute_rewards(&self) {
        let epoch = match self.metadata.get(&Metadata::Epoch) {
            Some(Value::Epoch(epoch)) => epoch,
            _ => 0,
        };

        let reward_pool = self
            .total_served
            .get(&epoch)
            .unwrap_or_default()
            .reward_pool;

        // if reward is 0, no commodity under any service was served
        if reward_pool == HpUfixed::zero() {
            return;
        }

        let node_percentage: HpUfixed<18> = self
            .parameters
            .get(&ProtocolParams::NodeShare)
            .unwrap_or_default()
            .into();
        let node_share = &node_percentage / &(*BIG_HUNDRED);
        let emissions = self.calculate_emissions();
        let emissions_for_node = &emissions * &node_share;

        let mut total_reward_share: HpUfixed<18> = HpUfixed::from(0_u64);
        let mut local_shares_map: HashMap<NodeIndex, HpUfixed<18>> = HashMap::new();
        let mut node_info_map: HashMap<NodeIndex, NodeInfo> = HashMap::new();

        for node in self.current_epoch_served.keys() {
            // safe to unwrap since all the nodes in current_epoch_served table are in node info
            // this is checked in submit_pod contract/function
            let node_info = self.node_info.get(&node).unwrap();
            node_info_map.insert(node, node_info.clone());

            let stables_revenue: HpUfixed<6> = self
                .current_epoch_served
                .get(&node)
                .unwrap_or_default()
                .stables_revenue;

            let node_service_proportion =
                &stables_revenue.convert_precision::<18>() / &reward_pool.convert_precision::<18>();
            self.mint_and_transfer_stables(
                stables_revenue * &node_share.convert_precision(),
                node_info.owner,
            );

            let locked_until = node_info.stake.stake_locked_until;
            let local_boost: HpUfixed<3> = self.get_boost(locked_until, &epoch);
            let local_share = node_service_proportion * &local_boost.convert_precision();
            total_reward_share = total_reward_share + &local_share;
            local_shares_map.insert(node, local_share);
        }

        let base_reward = &emissions_for_node / &total_reward_share;

        for (node, node_info) in node_info_map.iter() {
            let local_share = local_shares_map.get(node).unwrap();
            let flk_rewards = &base_reward * local_share;

            // todo: add service builders and protocols share in stables too
            self.mint_and_transfer_flk(flk_rewards, node_info.owner);
            self.current_epoch_served.remove(node);
        }

        // todo: add service builders revenue
        let service_share: HpUfixed<18> = &(self
            .parameters
            .get(&ProtocolParams::ServiceBuilderShare)
            .unwrap_or_default()
            .into())
            / &(*BIG_HUNDRED);
        let services_stable_reward_pool = &reward_pool * &service_share.convert_precision();
        let services_flk_reward_pool = &emissions * &service_share;
        for service_id in self.service_revenue.keys() {
            let service_owner = self.services.get(&service_id).unwrap().owner;
            let service_revenue = self.service_revenue.get(&service_id).unwrap_or_default();
            let revenue_proportion: HpUfixed<18> =
                &service_revenue.convert_precision() / &reward_pool.convert_precision();
            self.mint_and_transfer_stables(
                &services_stable_reward_pool * &revenue_proportion.convert_precision(),
                service_owner,
            );
            self.mint_and_transfer_flk(
                &services_flk_reward_pool * &revenue_proportion.convert_precision(),
                service_owner,
            );
            self.service_revenue.remove(&service_id);
        }

        // protocols share for rewards
        let protocol_share: HpUfixed<18> = &(self
            .parameters
            .get(&ProtocolParams::ProtocolShare)
            .unwrap_or_default()
            .into())
            / &(*BIG_HUNDRED);

        let protocol_owner = match self.metadata.get(&Metadata::ProtocolFundAddress) {
            Some(Value::AccountPublicKey(owner)) => owner,
            _ => panic!("ProtocolFundAddress is added at Genesis and should exist"),
        };
        self.mint_and_transfer_stables(
            &reward_pool * &protocol_share.convert_precision(),
            protocol_owner,
        );
        self.mint_and_transfer_flk(&emissions * &protocol_share, protocol_owner);
    }

    fn calculate_emissions(&self) -> HpUfixed<18> {
        let percentage_divisor = HpUfixed::<18>::from(100_u64);
        let inflation_percent: HpUfixed<18> = self
            .parameters
            .get(&ProtocolParams::MaxInflation)
            .unwrap_or(0)
            .into();
        let inflation: HpUfixed<18> =
            (&inflation_percent / &percentage_divisor).convert_precision();

        let supply_at_year_start = match self.metadata.get(&Metadata::SupplyYearStart) {
            Some(Value::HpUfixed(supply)) => supply,
            _ => panic!("SupplyYearStart is set genesis and should never be empty"),
        };

        (&inflation * &supply_at_year_start.convert_precision()) / &365.0.into()
    }

    fn mint_and_transfer_stables(&self, amount: HpUfixed<6>, owner: EthAddress) {
        let mut account = self.account_info.get(&owner).unwrap_or_default();

        account.stables_balance += amount;
        self.account_info.set(owner, account);
    }

    fn mint_and_transfer_flk(&self, amount: HpUfixed<18>, owner: EthAddress) {
        let mut account = self.account_info.get(&owner).unwrap_or_default();
        account.flk_balance += amount.clone();

        self.account_info.set(owner, account);

        let mut current_supply = match self.metadata.get(&Metadata::TotalSupply) {
            Some(Value::HpUfixed(supply)) => supply,
            _ => panic!("TotalSupply is set genesis and should never be empty"),
        };

        current_supply += amount;
        self.metadata.set(
            Metadata::TotalSupply,
            Value::HpUfixed(current_supply.clone()),
        );

        let current_epoch = match self.metadata.get(&Metadata::Epoch) {
            Some(Value::Epoch(epoch)) => epoch,
            _ => 0,
        };

        if (current_epoch + 1) % 365_u64 == 0_u64 {
            self.metadata
                .set(Metadata::SupplyYearStart, Value::HpUfixed(current_supply));
        }
    }

    /// The `get_boost` method calculates a reward boost factor based on the locked staking period.
    ///
    /// This function follows a linear interpolation mathematical model given by the equation:
    /// boost(t) = min(maxBoost, minBoost + (maxBoost - minBoost) * (t/max_T))
    /// The locked period is determined by the difference between the current
    /// epoch and the 'locked_until' parameter, signifying the epoch until which the stake is
    /// locked.
    ///
    /// As the stake nears its expiry, the reward boost decreases according to the function.
    ///
    /// The calculated boost value is capped at `maxBoost`, a parameter set by the protocol, to
    /// prevent excessive boosts. The boost formula is as follows:
    ///
    /// The `minBoost` value is consistently set at 1, as we aim to avoid penalizing nodes that are
    /// not participating in staking lockups.
    ///
    /// Parameters:
    /// * `locked_until`: The epoch number when the node's stake will be unlocked.
    /// * `current_epoch`: The current epoch number.
    ///
    /// Returns:
    /// - The calculated boost factor for the current rewards.
    fn get_boost(&self, locked_until: u64, current_epoch: &Epoch) -> HpUfixed<3> {
        let max_boost: HpUfixed<3> = self
            .parameters
            .get(&ProtocolParams::MaxBoost)
            .unwrap_or(1)
            .into();
        let max_lock_time: HpUfixed<3> = self
            .parameters
            .get(&ProtocolParams::MaxStakeLockTime)
            .unwrap_or(1)
            .into();
        let min_boost = HpUfixed::from(1_u64);
        let locking_period: HpUfixed<3> = (locked_until.saturating_sub(*current_epoch)).into();
        let boost = &min_boost + (&max_boost - &min_boost) * (&locking_period / &max_lock_time);
        HpUfixed::<3>::min(&max_boost, &boost).to_owned()
    }

    fn choose_new_committee(&self) -> Vec<NodeIndex> {
        let epoch = match self.metadata.get(&Metadata::Epoch) {
            Some(Value::Epoch(epoch)) => epoch,
            _ => 0,
        };

        let mut node_registry: Vec<NodeIndex> = self
            .get_node_registry()
            .keys()
            .copied()
            // The unwrap is safe because `node_registry` contains only valid node indices.
            .filter(|index| self.node_info.get(index).unwrap().participation == Participation::True)
            .collect();
        let committee_size = self.parameters.get(&ProtocolParams::CommitteeSize).unwrap();
        let num_of_nodes = node_registry.len() as u128;
        // if total number of nodes are less than committee size, all nodes are part of committee
        if committee_size >= num_of_nodes {
            return node_registry;
        }

        let committee = self.committee_info.get(&epoch).unwrap();
        let epoch_end = committee.epoch_end_timestamp;
        let public_key = {
            if !committee.members.is_empty() {
                let mid_index = committee.members.len() / 2;
                self.node_info
                    .get(&committee.members[mid_index])
                    .unwrap()
                    .public_key
            } else {
                NodePublicKey([1u8; 32])
            }
        };

        let mut hasher = Hasher::new();
        hasher.update(&public_key.0);
        hasher.update(&epoch_end.to_be_bytes());
        let result = hasher.finalize();
        let mut seed = [0u8; 32];
        seed.copy_from_slice(&result.as_bytes()[0..32]);
        let mut rng: StdRng = SeedableRng::from_seed(seed);
        node_registry.shuffle(&mut rng);
        node_registry
            .into_iter()
            .take(committee_size.try_into().unwrap())
            .collect()
    }

    /// This function takes in the Transaction and verifies the Signature matches the Sender. It
    /// also checks the nonce of the sender and makes sure it is equal to the account nonce + 1,
    /// to prevent replay attacks and enforce ordering
    pub fn verify_transaction(&self, txn: &mut TransactionRequest) -> Result<(), ExecutionError> {
        match txn {
            TransactionRequest::UpdateRequest(payload) => self.verify_fleek_transaction(payload),
            TransactionRequest::EthereumRequest(payload) => {
                self.verify_ethereum_transaction(payload.deref_mut())
            },
        }
    }

    fn verify_fleek_transaction(&self, txn: &UpdateRequest) -> Result<(), ExecutionError> {
        // Check nonce
        match txn.payload.sender {
            // Todo Sunday(dalton): Clean up this match nesting
            TransactionSender::NodeMain(node) => {
                if let Some(index) = self.pub_key_to_index.get(&node) {
                    if let Some(info) = self.node_info.get(&index) {
                        if txn.payload.nonce != info.nonce + 1 {
                            return Err(ExecutionError::InvalidNonce);
                        }
                    } else {
                        return Err(ExecutionError::NodeDoesNotExist);
                    }
                } else {
                    return Err(ExecutionError::NodeDoesNotExist);
                }
            },
            TransactionSender::NodeConsensus(node) => {
                if let Some(index) = self.consensus_key_to_index.get(&node) {
                    if let Some(info) = self.node_info.get(&index) {
                        if txn.payload.nonce != info.nonce + 1 {
                            return Err(ExecutionError::InvalidNonce);
                        }
                    }
                } else {
                    return Err(ExecutionError::NodeDoesNotExist);
                }
            },
            TransactionSender::AccountOwner(account) => {
                let account_info = self.account_info.get(&account).unwrap_or_default();
                if txn.payload.nonce != account_info.nonce + 1 {
                    return Err(ExecutionError::InvalidNonce);
                }
            },
        }

        // Check signature
        let payload = txn.payload.clone();
        let digest = payload.to_digest();

        if !txn.payload.sender.verify(txn.signature, &digest) {
            return Err(ExecutionError::InvalidSignature);
        }
        Ok(())
    }

    fn verify_ethereum_transaction(
        &self,
        txn: &mut EthersTransaction,
    ) -> Result<(), ExecutionError> {
        // Recover public key from signature
        // todo(Dalton) I am pretty sure this right here is enough to verify the signature and the
        //      extra verify we do at the bottom is unneeded
        let sender: EthAddress = if let Ok(address) = txn.recover_from_mut() {
            address.0.into()
        } else {
            return Err(ExecutionError::InvalidSignature);
        };

        // Verify nonce is correct
        let account_info = self.account_info.get(&sender).unwrap_or_default();
        if txn.nonce.as_u64() != account_info.nonce + 1 {
            return Err(ExecutionError::InvalidNonce);
        }

        if lightning_utils::eth::verify_signature(txn, sender) {
            Ok(())
        } else {
            Err(ExecutionError::InvalidSignature)
        }
    }

    /// Takes in a zk Proof Of Delivery and returns true if valid
    fn verify_proof_of_delivery(
        &self,
        _client: &EthAddress,
        _provider: &NodeIndex,
        _commodity: &u128,
        _service_id: &u32,
        _proof: (),
    ) -> bool {
        true
    }

    /// Takes in a zk Proof Of Consensus and returns true if valid
    fn verify_proof_of_consensus(&self, _proof: ProofOfConsensus) -> bool {
        true
    }

    /// Creates a new node. A new node should only be created through this function.
    fn create_node(&self, node: NodeInfo) -> bool {
        // If this public key or network key is already indexed to no create it
        if self.pub_key_to_index.get(&node.public_key).is_some()
            || self
                .consensus_key_to_index
                .get(&node.consensus_key)
                .is_some()
        {
            return false;
        }
        let node_index = match self.metadata.get(&Metadata::NextNodeIndex) {
            Some(Value::NextNodeIndex(index)) => index,
            _ => 0,
        };
        self.pub_key_to_index.set(node.public_key, node_index);
        self.consensus_key_to_index
            .set(node.consensus_key, node_index);

        self.node_info.set(node_index, node);
        self.metadata.set(
            Metadata::NextNodeIndex,
            Value::NextNodeIndex(node_index + 1),
        );
        true
    }

    /// Remove a node. A node should only be removed through this function.
    #[allow(unused)]
    fn remove_node(&self, node: NodePublicKey) -> bool {
        if let Some(index) = self.pub_key_to_index.get(&node) {
            if let Some(info) = self.node_info.get(&index) {
                self.consensus_key_to_index.remove(&info.consensus_key)
            }
            self.node_info.remove(&index);
            self.pub_key_to_index.remove(&node);
            true
        } else {
            false
        }
    }

    /// Called internally at the end of every transaction to increment the senders nonce.
    /// This happens even if the transaction reverts unless it reverts do to an invalid signature or
    /// nonce This function reverts if the sender does not exist so should be created before
    /// calling this function
    fn increment_nonce(&self, sender: TransactionSender) {
        match sender {
            TransactionSender::NodeMain(node) => {
                let index = self.pub_key_to_index.get(&node).unwrap();
                let mut node_info = self.node_info.get(&index).unwrap();
                node_info.nonce += 1;
                self.node_info.set(index, node_info);
            },
            TransactionSender::NodeConsensus(node) => {
                let index = self.consensus_key_to_index.get(&node).unwrap();
                let mut node_info = self.node_info.get(&index).unwrap();
                node_info.nonce += 1;
                self.node_info.set(index, node_info);
            },
            TransactionSender::AccountOwner(account) => {
                let mut account_info = self.account_info.get(&account).unwrap_or_default();
                account_info.nonce += 1;
                self.account_info.set(account, account_info);
            },
        }
    }

    /// Increments block number after executing a block and returns the new number
    pub fn increment_block_number(&self) -> u64 {
        let new_block_number = self.get_block_number() + 1;

        self.metadata
            .set(Metadata::BlockNumber, Value::BlockNumber(new_block_number));
        new_block_number
    }

    // Useful for transaction that nodes cannot call but an account owner can
    // Does not panic
    fn only_account_owner(
        &self,
        sender: TransactionSender,
    ) -> Result<EthAddress, TransactionResponse> {
        match sender {
            TransactionSender::AccountOwner(account) => Ok(account),
            _ => Err(TransactionResponse::Revert(
                ExecutionError::OnlyAccountOwner,
            )),
        }
    }
    // Useful for transaction that nodes can call but an account owner can't
    // Does not panic
    fn only_node(&self, sender: TransactionSender) -> Result<NodeIndex, TransactionResponse> {
        let node_index = match sender {
            TransactionSender::NodeMain(public_key) => match self.pub_key_to_index.get(&public_key)
            {
                Some(node_index) => node_index,
                None => {
                    return Err(TransactionResponse::Revert(
                        ExecutionError::NodeDoesNotExist,
                    ));
                },
            },
            TransactionSender::NodeConsensus(public_key) => {
                match self.consensus_key_to_index.get(&public_key) {
                    Some(node_index) => node_index,
                    None => {
                        return Err(TransactionResponse::Revert(
                            ExecutionError::NodeDoesNotExist,
                        ));
                    },
                }
            },

            _ => return Err(TransactionResponse::Revert(ExecutionError::OnlyNode)),
        };
        if !self.is_valid_node(&node_index).unwrap_or(false) {
            return Err(TransactionResponse::Revert(
                ExecutionError::InsufficientStake,
            ));
        }
        Ok(node_index)
    }

    // Checks if a node has staked the minimum required amount.
    // Returns `None` if the
    //
    // Panics:
    // This function can panic if ProtocolParams::MinimumNodeStake is missing from the parameters.
    // This should not happen because this parameter is seeded in the genesis.
    fn is_valid_node(&self, node_index: &NodeIndex) -> Option<bool> {
        let min_amount = self
            .parameters
            .get(&ProtocolParams::MinimumNodeStake)
            .unwrap();
        self.node_info
            .get(node_index)
            .map(|node_info| node_info.stake.staked >= min_amount.into())
    }

    fn get_node_info(&self, sender: TransactionSender) -> Option<(NodeIndex, NodeInfo)> {
        match sender {
            TransactionSender::NodeMain(public_key) => match self.pub_key_to_index.get(&public_key)
            {
                Some(index) => self.node_info.get(&index).map(|info| (index, info)),
                None => None,
            },
            TransactionSender::NodeConsensus(public_key) => {
                match self.consensus_key_to_index.get(&public_key) {
                    Some(index) => self.node_info.get(&index).map(|info| (index, info)),
                    None => None,
                }
            },
            TransactionSender::AccountOwner(_) => None,
        }
    }

    pub fn get_block_number(&self) -> u64 {
        // Safe unwrap this value is set on genesis and never removed
        if let Value::BlockNumber(block_number) = self.metadata.get(&Metadata::BlockNumber).unwrap()
        {
            block_number
        } else {
            // unreachable
            0
        }
    }

    pub fn get_block_hash(&self) -> [u8; 32] {
        if let Some(Value::Hash(hash)) = self.metadata.get(&Metadata::LastBlockHash) {
            hash
        } else {
            // unreachable set at genesis
            [0; 32]
        }
    }

    fn _get_epoch(&self) -> u64 {
        if let Some(Value::Epoch(epoch)) = self.metadata.get(&Metadata::Epoch) {
            epoch
        } else {
            // unreachable set at genesis
            0
        }
    }

    fn clear_content_registry(&self, node_index: &NodeIndex) -> Result<(), ExecutionError> {
        let cids = self.node_to_cid.get(node_index).unwrap_or_default();

        // Let's stage the changes before applying.
        let mut staged_cid_to_node = HashMap::new();
        for cid in cids {
            let mut providers = self
                .cid_to_node
                .get(&cid)
                .ok_or(ExecutionError::InvalidStateForContentRemoval)?;
            if !providers.remove(node_index) {
                return Err(ExecutionError::InvalidStateForContentRemoval);
            }
            staged_cid_to_node.insert(cid, providers);
        }

        // Apply changes.
        for (cid, providers) in staged_cid_to_node {
            self.cid_to_node.set(cid, providers);
        }
        self.node_to_cid.remove(node_index);

        Ok(())
    }

    fn clean_up_content_registry(&self) {
        for (index, info) in self.get_node_registry() {
            if matches!(info.participation, Participation::False) {
                // Note: Unless there is a bug in the content registry system,
                // this operation should not fail.
                let _ = self.clear_content_registry(&index);
            }
        }
    }
}
