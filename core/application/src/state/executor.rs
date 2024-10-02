use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::net::IpAddr;
use std::ops::DerefMut;
use std::time::Duration;

use ethers::abi::AbiDecode;
use ethers::types::{Transaction as EthersTransaction, H160};
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
    CommitteeSelectionBeaconCommit,
    CommitteeSelectionBeaconReveal,
    CommodityTypes,
    ContentUpdate,
    DeliveryAcknowledgmentProof,
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
    ProtocolParamKey,
    ProtocolParamValue,
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
    MAX_UPDATES_CONTENT_REGISTRY,
};
use lightning_interfaces::ToDigest;
use lightning_utils::eth::fleek_contract::FleekContractCalls;
use lightning_utils::eth::{
    DepositCall,
    StakeCall,
    UnstakeCall,
    WithdrawCall,
    WithdrawUnstakedCall,
};

use super::context::{Backend, TableRef};

mod epoch_change;

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

lazy_static! {
    static ref BIG_HUNDRED: HpUfixed<18> = HpUfixed::<18>::from(100_u64);
}

/// The state executor encapsuates the logic for executing transactions.
///
/// The functions implemented on this struct are the "Smart Contracts" of the application layer
/// All state changes come from Transactions and start at execute_transaction
pub struct StateExecutor<B: Backend> {
    pub metadata: B::Ref<Metadata, Value>,
    pub account_info: B::Ref<EthAddress, AccountInfo>,
    pub client_keys: B::Ref<ClientPublicKey, EthAddress>,
    pub node_info: B::Ref<NodeIndex, NodeInfo>,
    pub consensus_key_to_index: B::Ref<ConsensusPublicKey, NodeIndex>,
    pub pub_key_to_index: B::Ref<NodePublicKey, NodeIndex>,
    pub latencies: B::Ref<(NodeIndex, NodeIndex), Duration>,
    pub committee_info: B::Ref<Epoch, Committee>,
    pub services: B::Ref<ServiceId, Service>,
    pub parameters: B::Ref<ProtocolParamKey, ProtocolParamValue>,
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
    pub uri_to_node: B::Ref<Blake3Hash, BTreeSet<NodeIndex>>,
    pub node_to_uri: B::Ref<NodeIndex, BTreeSet<Blake3Hash>>,
    pub committee_selection_beacon: B::Ref<
        NodeIndex,
        (
            CommitteeSelectionBeaconCommit,
            Option<CommitteeSelectionBeaconReveal>,
        ),
    >,
    pub backend: B,
}

impl<B: Backend> StateExecutor<B> {
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
            uri_to_node: backend.get_table_reference("uri_to_node"),
            node_to_uri: backend.get_table_reference("node_to_uri"),
            committee_selection_beacon: backend.get_table_reference("committee_selection_beacon"),
            backend,
        }
    }

    /// Executes a generic transaction.
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

    /// Executes a fleek transaction.
    fn execute_fleek_transaction(&self, txn: UpdateRequest) -> TransactionResponse {
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

            UpdateMethod::CommitteeSelectionBeaconCommit { commit } => {
                self.committee_selection_beacon_commit(txn.payload.sender, commit)
            },

            UpdateMethod::CommitteeSelectionBeaconReveal { reveal } => {
                self.committee_selection_beacon_reveal(txn.payload.sender, reveal)
            },

            UpdateMethod::CommitteeSelectionBeaconCommitPhaseTimeout => {
                self.committee_selection_beacon_commit_phase_timeout(txn.payload.sender)
            },

            UpdateMethod::CommitteeSelectionBeaconRevealPhaseTimeout => {
                self.committee_selection_beacon_reveal_phase_timeout(txn.payload.sender)
            },

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
            UpdateMethod::IncrementNonce {} => TransactionResponse::Success(ExecutionData::None),
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

    /// Executes an ethereum transaction.
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
        _acknowledgments: Vec<DeliveryAcknowledgmentProof>,
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

        let max_lock_time = match self.parameters.get(&ProtocolParamKey::MaxStakeLockTime) {
            Some(ProtocolParamValue::MaxStakeLockTime(v)) => v,
            _ => 1,
        };

        // check if total locking is greater than max locking
        if current_lock + locked_for > max_lock_time {
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

        let lock_time = match self.parameters.get(&ProtocolParamKey::LockTime) {
            Some(ProtocolParamValue::LockTime(v)) => v,
            _ => 0,
        };

        // decrease the stake, add to the locked amount, and set the locked time for the withdrawal
        // current epoch + lock time todo(dalton): we should be storing unstaked tokens in a
        // list so we can have multiple locked stakes with dif lock times
        node.stake.staked -= amount.clone();
        node.stake.locked += amount;
        node.stake.locked_until = current_epoch + lock_time;

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

    // This method can panic if the governance address wasn't previously stored in the application
    // state. The governance address should be seeded though the genesis.
    fn change_protocol_param(
        &self,
        sender: TransactionSender,
        param: ProtocolParamKey,
        value: ProtocolParamValue,
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
        self.node_info
            .keys()
            .filter_map(|key| self.node_info.get(&key).map(|node| (key, node)))
            .filter(|(_, node)| node.stake.staked > HpUfixed::zero())
            .collect()
    }

    // This function should only be called in the `run` method on `Env`.
    pub fn set_last_epoch_hash(&self, state_hash: [u8; 32]) {
        self.metadata
            .set(Metadata::LastEpochHash, Value::Hash(state_hash));
    }

    // This function should only be called in the `run` method on `Env`.
    pub fn set_last_block(&self, block_hash: [u8; 32], sub_dag_index: u64, sub_dag_round: u64) {
        self.metadata
            .set(Metadata::LastBlockHash, Value::Hash(block_hash));
        self.metadata
            .set(Metadata::SubDagIndex, Value::SubDagIndex(sub_dag_index));
        self.metadata
            .set(Metadata::SubDagRound, Value::SubDagRound(sub_dag_round));
        self.metadata.set(
            Metadata::BlockNumber,
            Value::BlockNumber(self.get_last_block_number() + 1),
        );
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
                    let mut node_measurements =
                        self.rep_measurements.get(&peer_index).unwrap_or_default();
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

        let mut staged_uri_to_node = HashMap::new();
        let mut staged_uris_provided = self.node_to_uri.get(&node_index).unwrap_or_default();
        let empty_uris_provided = staged_uris_provided.is_empty();

        for update in updates {
            // Check if they sent multiple updates for the same URI.
            if staged_uri_to_node.contains_key(&update.uri) {
                return TransactionResponse::Revert(ExecutionError::TooManyUpdatesForContent);
            }

            let providers = self.uri_to_node.get(&update.uri);

            // Check if a removal request makes sense given our state.
            if update.remove && (providers.is_none() || empty_uris_provided) {
                return TransactionResponse::Revert(ExecutionError::InvalidStateForContentRemoval);
            }

            let mut providers = providers.unwrap_or_default();

            if update.remove {
                if !providers.remove(&node_index) {
                    return TransactionResponse::Revert(ExecutionError::InvalidContentRemoval);
                }
                if !staged_uris_provided.remove(&update.uri) {
                    // Unreachable.
                    return TransactionResponse::Revert(ExecutionError::InvalidContentRemoval);
                }
            } else {
                providers.insert(node_index);
                staged_uris_provided.insert(update.uri);
            }

            staged_uri_to_node.insert(update.uri, providers);
        }

        self.node_to_uri.set(node_index, staged_uris_provided);

        for (uri, providers) in staged_uri_to_node {
            self.uri_to_node.set(uri, providers);
        }

        TransactionResponse::Success(ExecutionData::None)
    }

    /********Internal Application Functions******** */
    // These functions should only ever be called in the context of an external transaction function
    // They should never panic and any check that could result in that should be done in the
    // external function that calls it The functions that should call this and the required
    // checks should be documented for each function

    /// Settles the auction for the current epoch and returns a list of the active set of nodes
    /// Uses quick sort algorithm for effecient sorting
    fn settle_auction(&self, nodes: Vec<(NodeIndex, NodeInfo)>) -> Vec<(NodeIndex, NodeInfo)> {
        let node_count = match self.parameters.get(&ProtocolParamKey::NodeCount) {
            Some(ProtocolParamValue::NodeCount(v)) => v as usize,
            _ => unreachable!(), // set in genesis
        };

        let total_nodes = nodes.len();

        if total_nodes <= node_count {
            return nodes;
        }
        // Quick sort algo will find smallest kth nodes so we need to subtract target node count
        // from total
        let k = total_nodes - node_count;
        let r = total_nodes - 1;

        self.quick_sort_nodes(nodes, 0, r, k)
    }

    /// Used by settle_auction. Modified quick sort algorithm that will give us the kth highest
    /// staked nodes with our own custom compare function where we can have custom tiebreakers for
    /// reputation and things of that natrue
    fn quick_sort_nodes(
        &self,
        mut nodes: Vec<(NodeIndex, NodeInfo)>,
        l: usize,
        r: usize,
        k: usize,
    ) -> Vec<(NodeIndex, NodeInfo)> {
        let pivot = self.partition_nodes(&mut nodes, l, r);

        match pivot.cmp(&(k - 1)) {
            Ordering::Equal => nodes[pivot + 1..].to_vec(),
            Ordering::Greater => self.quick_sort_nodes(nodes, l, pivot - 1, k),
            _ => self.quick_sort_nodes(nodes, pivot + 1, r, k),
        }
    }

    /// Partition part of the quick sort algorithm used to settle auctions
    fn partition_nodes(&self, nodes: &mut [(NodeIndex, NodeInfo)], l: usize, r: usize) -> usize {
        let pivot = nodes[r].clone();
        let mut i = l;

        for j in l..r {
            if self.compare_nodes_in_auction(&nodes[j], &pivot) {
                nodes.swap(j, i);
                i += 1;
            }
        }
        nodes.swap(i, r);

        i
    }

    /// The compare function in our auction settlement. Here we will have the logic that decides who
    /// wins the staking auction
    /// pos is the the current position in the partition and pivot is the current pivot being
    /// compared against should return true if current position is less than pivot
    fn compare_nodes_in_auction(
        &self,
        pos: &(NodeIndex, NodeInfo),
        pivot: &(NodeIndex, NodeInfo),
    ) -> bool {
        // todo(dalton): This is where we add tiebreakers like reputation score. Or modifiers on the
        // stake
        match pos.1.stake.staked.cmp(&pivot.1.stake.staked) {
            Ordering::Less => true,
            Ordering::Greater => false,
            Ordering::Equal => {
                // If they are equal fall back to whichever one has the highest reputation score
                // If pos is less than or equal we return true, otherwise false to so this is sorted
                // correctly
                self.rep_scores.get(&pos.0).unwrap_or_default()
                    <= self.rep_scores.get(&pivot.0).unwrap_or_default()
            },
        }
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

        let epoch_per_year = self.get_epochs_per_year();

        current_supply += amount;
        self.metadata.set(
            Metadata::TotalSupply,
            Value::HpUfixed(current_supply.clone()),
        );

        let current_epoch = match self.metadata.get(&Metadata::Epoch) {
            Some(Value::Epoch(epoch)) => epoch,
            _ => 0,
        };

        if (current_epoch + 1) % epoch_per_year == 0_u64 {
            self.metadata
                .set(Metadata::SupplyYearStart, Value::HpUfixed(current_supply));
        }
    }

    fn get_epochs_per_year(&self) -> u64 {
        match self
            .parameters
            .get(&ProtocolParamKey::EpochsPerYear)
            .unwrap()
        {
            ProtocolParamValue::EpochsPerYear(v) => v,
            _ => unreachable!("invalid epoch per year in parameters"),
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
        let max_boost = match self.parameters.get(&ProtocolParamKey::MaxBoost) {
            Some(ProtocolParamValue::MaxBoost(v)) => HpUfixed::<3>::from(v),
            _ => HpUfixed::<3>::from(1_u16), // set in genesis
        };

        let max_lock_time = match self.parameters.get(&ProtocolParamKey::MaxStakeLockTime) {
            Some(ProtocolParamValue::MaxStakeLockTime(v)) => HpUfixed::<3>::from(v),
            _ => HpUfixed::<3>::from(1_u16), // set in genesis
        };

        let min_boost = HpUfixed::from(1_u64);
        let locking_period: HpUfixed<3> = (locked_until.saturating_sub(*current_epoch)).into();
        let boost = &min_boost + (&max_boost - &min_boost) * (&locking_period / &max_lock_time);
        HpUfixed::<3>::min(&max_boost, &boost).to_owned()
    }

    /// This function takes in the Transaction and verifies the Signature matches the Sender. It
    /// also checks the nonce of the sender and makes sure it is equal to the account nonce + 1,
    /// to prevent replay attacks and enforce ordering. Additionally, it verifies ChainID
    /// with the current value from Metadata Table

    pub fn verify_transaction(&self, txn: &mut TransactionRequest) -> Result<(), ExecutionError> {
        self.verify_chain_id(txn)?;

        match txn {
            TransactionRequest::UpdateRequest(payload) => self.verify_fleek_transaction(payload),
            TransactionRequest::EthereumRequest(payload) => {
                self.verify_ethereum_transaction(payload.deref_mut())
            },
        }
    }

    fn verify_chain_id(&self, txn: &TransactionRequest) -> Result<(), ExecutionError> {
        match self.metadata.get(&Metadata::ChainId) {
            Some(Value::ChainId(chain_id)) => {
                if chain_id == txn.chain_id() {
                    Ok(())
                } else {
                    Err(ExecutionError::InvalidChainId)
                }
            },
            _ => Ok(()),
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
        let new_block_number = self.get_last_block_number() + 1;

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
        let min_amount = match self.parameters.get(&ProtocolParamKey::MinimumNodeStake) {
            Some(ProtocolParamValue::MinimumNodeStake(v)) => v,
            _ => unreachable!(), // set in genesis
        };

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

    pub fn get_last_block_number(&self) -> u64 {
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

    /// Gets subdag index, returns 0 if not set in state table
    pub fn get_sub_dag_index(&self) -> u64 {
        if let Some(Value::SubDagIndex(value)) = self.metadata.get(&Metadata::SubDagIndex) {
            value
        } else {
            0
        }
    }

    /// Gets subdag round, returns 0 if not set in state table
    pub fn get_sub_dag_round(&self) -> u64 {
        if let Some(Value::SubDagRound(value)) = self.metadata.get(&Metadata::SubDagRound) {
            value
        } else {
            0
        }
    }

    fn get_epoch(&self) -> u64 {
        if let Some(Value::Epoch(epoch)) = self.metadata.get(&Metadata::Epoch) {
            epoch
        } else {
            // unreachable set at genesis
            0
        }
    }

    fn clear_content_registry(&self, node_index: &NodeIndex) -> Result<(), ExecutionError> {
        let uris = self.node_to_uri.get(node_index).unwrap_or_default();

        // Let's stage the changes before applying.
        let mut staged_uri_to_node = HashMap::new();
        for uri in uris {
            let mut providers = self
                .uri_to_node
                .get(&uri)
                .ok_or(ExecutionError::InvalidStateForContentRemoval)?;
            if !providers.remove(node_index) {
                return Err(ExecutionError::InvalidStateForContentRemoval);
            }
            staged_uri_to_node.insert(uri, providers);
        }

        // Apply changes.
        for (uri, providers) in staged_uri_to_node {
            self.uri_to_node.set(uri, providers);
        }
        self.node_to_uri.remove(node_index);

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
