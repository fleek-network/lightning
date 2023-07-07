use std::{
    cmp::Ordering,
    collections::{BTreeMap, HashMap},
};

use draco_interfaces::{
    types::{
        AccountInfo, CommodityServed, CommodityTypes, Epoch, ExecutionData, ExecutionError,
        Metadata, NodeInfo, ProofOfConsensus, ProofOfMisbehavior, ProtocolParams,
        ReportedReputationMeasurements, ReputationMeasurements, Service, ServiceId, Staking,
        Tokens, TotalServed, TransactionResponse, UpdateMethod, UpdateRequest, Value, Worker,
    },
    DeliveryAcknowledgment, ToDigest,
};
use draco_reputation::{statistics, types::WeightedReputationMeasurements};
use fleek_crypto::{
    ClientPublicKey, EthAddress, NodeNetworkingPublicKey, NodePublicKey, PublicKey,
    TransactionSender, TransactionSignature,
};
use hp_float::unsigned::HpUfloat;
use multiaddr::Multiaddr;
use num_traits::FromPrimitive;
use serde::{Deserialize, Serialize};

use crate::table::{Backend, TableRef};

/// Minimum number of reported measurements that have to be available for a node.
/// If less measurements have been reported, no reputation score will be computed in that epoch.
#[cfg(all(not(test), not(feature = "test")))]
const MIN_NUM_MEASUREMENTS: usize = 10;
#[cfg(any(test, feature = "test"))]
const MIN_NUM_MEASUREMENTS: usize = 2;

/// Reported measurements are weighted by the reputation score of the reporting node.
/// If there is no reputation score for the reporting node, we use a quantile from the array
/// of all reputation scores.
/// For example, if `DEFAULT_REP_QUANTILE = 0.3`, then we use the reputation score that is higher
/// than 30% of scores and lower than 70% of scores.
const DEFAULT_REP_QUANTILE: f64 = 0.3;

/// The rep score of a node is the exponentially weighted moving average of its rep scores over the
/// past epochs.
/// For example, `REP_EWMA_WEIGHT=0.7` means that 70% of the current rep score is based on past
/// epochs and 30% is based on the current epoch.
const REP_EWMA_WEIGHT: f64 = 0.7;

/// The state of the Application
///
/// The functions implemented on this struct are the "Smart Contracts" of the application layer
/// All state changes come from Transactions and start at execute_txn
pub struct State<B: Backend> {
    pub metadata: B::Ref<Metadata, Value>,
    pub account_info: B::Ref<EthAddress, AccountInfo>,
    pub client_keys: B::Ref<ClientPublicKey, EthAddress>,
    pub node_info: B::Ref<NodePublicKey, NodeInfo>,
    pub committee_info: B::Ref<Epoch, Committee>,
    pub services: B::Ref<ServiceId, Service>,
    pub parameters: B::Ref<ProtocolParams, u128>,
    pub rep_measurements: B::Ref<NodePublicKey, Vec<ReportedReputationMeasurements>>,
    pub rep_scores: B::Ref<NodePublicKey, u8>,
    pub current_epoch_served: B::Ref<NodePublicKey, CommodityServed>,
    pub last_epoch_served: B::Ref<NodePublicKey, CommodityServed>,
    pub total_served: B::Ref<Epoch, TotalServed>,
    pub commodity_prices: B::Ref<CommodityTypes, HpUfloat<6>>,
    pub backend: B,
}

#[derive(Debug, Hash, PartialEq, PartialOrd, Ord, Eq, Serialize, Deserialize, Clone, Default)]
pub struct Committee {
    pub members: Vec<NodePublicKey>,
    pub ready_to_change: Vec<NodePublicKey>,
    pub epoch_end_timestamp: u64,
}

impl<B: Backend> State<B> {
    pub fn new(backend: B) -> Self {
        Self {
            metadata: backend.get_table_reference("metadata"),
            account_info: backend.get_table_reference("account"),
            client_keys: backend.get_table_reference("client_keys"),
            node_info: backend.get_table_reference("node"),
            committee_info: backend.get_table_reference("committee"),
            services: backend.get_table_reference("service"),
            parameters: backend.get_table_reference("parameter"),
            rep_measurements: backend.get_table_reference("rep_measurements"),
            rep_scores: backend.get_table_reference("rep_scores"),
            last_epoch_served: backend.get_table_reference("last_epoch_served"),
            current_epoch_served: backend.get_table_reference("current_epoch_served"),
            total_served: backend.get_table_reference("total_served"),
            commodity_prices: backend.get_table_reference("commodity_prices"),
            backend,
        }
    }

    /// This function is the entry point of a transaction
    pub fn execute_txn(&self, txn: UpdateRequest) -> TransactionResponse {
        // Execute transaction
        let response = match txn.payload.method {
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
                amount,
                node_public_key,
                node_network_key,
                node_domain,
                worker_public_key,
                worker_domain,
                worker_mempool_address,
            } => self.stake(
                txn.sender,
                amount,
                node_public_key,
                node_network_key,
                node_domain,
                worker_public_key,
                worker_domain,
                worker_mempool_address,
            ),
            UpdateMethod::StakeLock { node, locked_for } => {
                self.stake_lock(txn.sender, node, locked_for)
            },

            UpdateMethod::Unstake { amount, node } => self.unstake(txn.sender, amount, node),

            UpdateMethod::WithdrawUnstaked { node, recipient } => {
                self.withdrawl_unstaked(txn.sender, node, recipient)
            },

            UpdateMethod::ChangeEpoch { epoch } => self.change_epoch(txn.sender, epoch),

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

            UpdateMethod::SubmitReputationMeasurements { measurements } => {
                self.submit_reputation_measurements(txn.sender, measurements)
            },
        };
        // Increment nonce of the sender
        self.increment_nonce(txn.sender);
        // Return the response
        response
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
        let sender: NodePublicKey = match self.only_node(sender) {
            Ok(node) => node,
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

        let mut commodity_served = self.current_epoch_served.get(&sender).unwrap_or_default();
        let mut total_served = self.total_served.get(&current_epoch).unwrap_or_default();
        let commodity_prices = self
            .commodity_prices
            .get(&commodity_type)
            .expect("Commodity price should always be set");

        let commodity_index = commodity_type as usize;
        for i in 0..=commodity_index {
            if i >= commodity_served.len() {
                commodity_served.push(0);
            }
            if i >= total_served.served.len() {
                total_served.served.push(0);
            }
        }
        commodity_served[commodity_index] += commodity;
        total_served.served[commodity_index] += commodity;
        let commodity_to_big: HpUfloat<6> = commodity.into();
        total_served.reward_pool += commodity_to_big * commodity_prices;

        self.current_epoch_served.set(sender, commodity_served);
        self.total_served.set(current_epoch, total_served);

        TransactionResponse::Success(ExecutionData::None)
    }

    fn withdraw(
        &self,
        _sender: TransactionSender,
        _reciever: EthAddress,
        _amount: HpUfloat<18>,
        _token: Tokens,
    ) -> TransactionResponse {
        todo!()
    }

    fn deposit(
        &self,
        sender: TransactionSender,
        proof: ProofOfConsensus,
        amount: HpUfloat<18>,
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

    #[allow(clippy::too_many_arguments)]
    fn stake(
        &self,
        sender: TransactionSender,
        amount: HpUfloat<18>,
        node_public_key: NodePublicKey,
        node_network_key: Option<NodeNetworkingPublicKey>,
        node_domain: Option<String>,
        worker_public_key: Option<NodeNetworkingPublicKey>,
        worker_domain: Option<String>,
        worker_mempool_address: Option<String>,
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

        // Make sure the sender has atleast the amount of FLK he is trying to stake
        if owner.flk_balance < amount {
            return TransactionResponse::Revert(ExecutionError::InsufficientBalance);
        }

        let node_domain = match node_domain {
            Some(address) => {
                let address = address.parse::<Multiaddr>();
                if address.is_err() {
                    return TransactionResponse::Revert(ExecutionError::InvalidInternetAddress);
                }
                Some(address.unwrap())
            },
            None => None,
        };

        let worker_domain = match worker_domain {
            Some(address) => {
                let address = address.parse::<Multiaddr>();
                if address.is_err() {
                    return TransactionResponse::Revert(ExecutionError::InvalidInternetAddress);
                }
                Some(address.unwrap())
            },
            None => None,
        };

        let worker_mempool_address = match worker_mempool_address {
            Some(address) => {
                let address = address.parse::<Multiaddr>();
                if address.is_err() {
                    return TransactionResponse::Revert(ExecutionError::InvalidInternetAddress);
                }
                Some(address.unwrap())
            },
            None => None,
        };

        let node = match self.node_info.get(&node_public_key) {
            Some(mut node) => {
                // Todo(dalton): should we stop people from staking on a node they do not own??

                // If any of the nodes fields were provided with this transaction update them on the
                // nodes state
                if let Some(network_key) = node_network_key {
                    node.network_key = network_key;
                }
                if let Some(primary_domain) = node_domain {
                    node.domain = primary_domain;
                }
                if let Some(worker_key) = worker_public_key {
                    node.workers[0].public_key = worker_key;
                }
                if let Some(worker_domain) = worker_domain {
                    node.workers[0].address = worker_domain
                }
                if let Some(mempool_address) = worker_mempool_address {
                    node.workers[0].mempool = mempool_address
                }

                // Increase the nodes stake by the amount being staked
                node.stake.staked += amount.clone();

                node
            },
            None => {
                // If the node doesnt Exist, create it. But check if they provided all the required
                // options for a new node
                if let (
                    Some(network_key),
                    Some(primary_domain),
                    Some(worker_key),
                    Some(worker_domain),
                    Some(mempool_domain),
                ) = (
                    node_network_key,
                    node_domain,
                    worker_public_key,
                    worker_domain,
                    worker_mempool_address,
                ) {
                    NodeInfo {
                        owner: sender,
                        public_key: node_public_key,
                        network_key,
                        staked_since: current_epoch,
                        stake: Staking {
                            staked: amount.clone(),
                            ..Default::default()
                        },
                        domain: primary_domain,
                        workers: [Worker {
                            public_key: worker_key,
                            address: worker_domain,
                            mempool: mempool_domain,
                        }]
                        .into(),
                        nonce: 0,
                    }
                } else {
                    return TransactionResponse::Revert(ExecutionError::InsufficientNodeDetails);
                }
            },
        };

        // decrement the owners balance
        owner.flk_balance -= amount;

        // Commit changes to the node and the owner
        self.account_info.set(sender, owner);
        self.node_info.set(node_public_key, node);

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

        let mut node = match self.node_info.get(&node_public_key) {
            Some(node) => node,
            None => return TransactionResponse::Revert(ExecutionError::NodeDoesNotExist),
        };

        // Make sure the caller is the owner of the node
        if sender != node.owner {
            return TransactionResponse::Revert(ExecutionError::NotNodeOwner);
        }
        // check if node has stakes to be locked
        if node.stake.staked == HpUfloat::zero() {
            return TransactionResponse::Revert(ExecutionError::InsufficientStakesToLock);
        }

        let epoch = match self.metadata.get(&Metadata::Epoch) {
            Some(Value::Epoch(epoch)) => epoch,
            _ => 0,
        };
        let current_lock = node.stake.stake_locked_until.saturating_sub(epoch);

        let max_lock_time = self
            .parameters
            .get(&ProtocolParams::MaxLockTime)
            .unwrap_or(1);

        // check if total locking is greater than max locking
        if current_lock + locked_for > max_lock_time as u64 {
            return TransactionResponse::Revert(ExecutionError::LockExceededMaxLockTime);
        }

        node.stake.stake_locked_until += locked_for;

        // Save the changed node state and return success
        self.node_info.set(node_public_key, node);
        TransactionResponse::Success(ExecutionData::None)
    }

    fn unstake(
        &self,
        sender: TransactionSender,
        amount: HpUfloat<18>,
        node_public_key: NodePublicKey,
    ) -> TransactionResponse {
        // This transaction is only callable by AccountOwners and not nodes
        // So revert if the sender is a node public key
        let sender = match self.only_account_owner(sender) {
            Ok(account) => account,
            Err(e) => return e,
        };

        let mut node = match self.node_info.get(&node_public_key) {
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

        // Make sure the node has atleast that much staked
        if node.stake.staked < amount {
            return TransactionResponse::Revert(ExecutionError::InsufficientBalance);
        }

        let lock_time = self.parameters.get(&ProtocolParams::LockTime).unwrap_or(0);

        // decrease the stake, add to the locked amount, and set the locked time for the withdrawl
        // current epoch + lock time todo(dalton): we should be storing unstaked tokens in a
        // list so we can have multiple locked stakes with dif lock times
        node.stake.staked -= amount.clone();
        node.stake.locked += amount;
        node.stake.locked_until = current_epoch + lock_time as u64;

        // Save the changed node state and return success
        self.node_info.set(node_public_key, node);
        TransactionResponse::Success(ExecutionData::None)
    }

    fn withdrawl_unstaked(
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

        let mut node = match self.node_info.get(&node_public_key) {
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
        if node.stake.locked == HpUfloat::zero() {
            return TransactionResponse::Revert(ExecutionError::NoLockedTokens);
        }
        if node.stake.locked_until > current_epoch {
            return TransactionResponse::Revert(ExecutionError::TokensLocked);
        }

        // if there is no recipient the owner will recieve the withdrawl
        let recipient = recipient.unwrap_or(sender_public_key);
        let mut reciever = self.account_info.get(&recipient).unwrap_or_default();

        // add the withdrawn tokens to the recipient and reset the nodes locked stake state
        // no need to reset locked_until on the node because that will get adjusted the next time
        // the node unstakes
        reciever.flk_balance += node.stake.locked;
        node.stake.locked = HpUfloat::zero();

        // Todo(dalton): if the nodes stake+locked are equal to 0 here should we remove him from the
        // state tables completly?

        // Save state changes and return response
        self.account_info.set(recipient, reciever);
        self.node_info.set(node_public_key, node);
        TransactionResponse::Success(ExecutionData::None)
    }

    fn change_epoch(&self, sender: TransactionSender, epoch: Epoch) -> TransactionResponse {
        // Only Nodes can call this function
        let sender = match self.only_node(sender) {
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
        if !current_committee.members.contains(&sender) {
            return TransactionResponse::Revert(ExecutionError::NotCommitteeMember);
        } else if current_committee.ready_to_change.contains(&sender) {
            return TransactionResponse::Revert(ExecutionError::AlreadySignaled);
        }
        current_committee.ready_to_change.push(sender);

        // If more than 2/3rds of the committee have signaled, start the epoch change process
        if current_committee.ready_to_change.len() > (2 * current_committee.members.len() / 3) {
            // Todo: Reward nodes, calculate rep?, choose new committee, increment epoch.
            self.calculate_reputation_scores();
            self.distribute_rewards();

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
            DEFAULT_REP_QUANTILE,
        )
        .unwrap_or(0);

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
        let new_rep_scores = draco_reputation::calculate_reputation_scores(map);
        // Store new scores in application state.
        new_rep_scores.iter().for_each(|(node, new_score)| {
            if let Some(old_score) = rep_scores.get(node) {
                let score = *old_score as f64 * REP_EWMA_WEIGHT
                    + (1.0 - REP_EWMA_WEIGHT) * *new_score as f64;
                self.rep_scores.set(*node, score as u8)
            } else {
                self.rep_scores.set(*node, *new_score);
            }
        });

        // If not in test mode, remove outdated rep scores.
        if cfg!(all(not(test), not(feature = "test"))) {
            let nodes = self.rep_scores.keys();
            nodes.for_each(|node| {
                if !new_rep_scores.contains_key(&node) {
                    self.rep_scores.remove(&node);
                }
            });
        }

        // Remove measurements from this epoch once we ccalculated the rep scores.
        let nodes = self.rep_measurements.keys();
        nodes.for_each(|node| self.rep_measurements.remove(&node));
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

    fn submit_reputation_measurements(
        &self,
        sender: TransactionSender,
        measurements: BTreeMap<NodePublicKey, ReputationMeasurements>,
    ) -> TransactionResponse {
        let reporting_node = match self.only_node(sender) {
            Ok(account) => account,
            Err(e) => return e,
        };
        measurements.into_iter().for_each(|(peer, measurements)| {
            let mut node_measurements = match self.rep_measurements.get(&peer) {
                Some(node_measurements) => node_measurements,
                None => Vec::new(),
            };
            node_measurements.push(ReportedReputationMeasurements {
                reporting_node,
                measurements,
            });
            self.rep_measurements.set(peer, node_measurements);
        });
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
    /// `FLk` emission per unit of stable earned is given by:
    ///
    /// `emission = (inflation * supply) / (maxBoost * rewardPool * daysInYear=365.0)`
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
        if reward_pool == HpUfloat::zero() {
            return;
        }

        let percentage_divisor = HpUfloat::<18>::from(100_u64);
        let node_percentage: HpUfloat<18> = self
            .parameters
            .get(&ProtocolParams::NodeShare)
            .unwrap_or_default()
            .into();
        let node_share = &node_percentage / &percentage_divisor;
        let (_max_emissions, min_emissions) = self.calculate_emissions();
        let mut total_emissions: HpUfloat<18> = 0.0.into();
        let nodes = self.current_epoch_served.keys();
        for node in nodes {
            let stables_revenue: HpUfloat<6> = self.calculate_node_revenue(&node);

            let node_owner = self.node_info.get(&node).unwrap().owner;
            self.mint_and_transfer_stables(stables_revenue.clone(), node_owner);

            // safe to unwrap since all the nodes in current_epoch_served table are in node info
            // this is checked in submit_pod contract/function
            let locked_until = self.node_info.get(&node).unwrap().stake.stake_locked_until;
            let boost: HpUfloat<18> = self.get_boost(locked_until, &epoch);

            let node_service_proportion =
                (&stables_revenue / &reward_pool).convert_precision::<18>();
            let flk_rewards = &min_emissions * node_service_proportion * &boost;
            total_emissions += flk_rewards.clone();
            let node_owner = self.node_info.get(&node).unwrap().owner;
            self.mint_and_transfer_flk(&flk_rewards * &node_share, node_owner);
            self.current_epoch_served.remove(&node);
        }

        // distribute rewards to validators
        let validator_share: HpUfloat<18> = self
            .parameters
            .get(&ProtocolParams::ValidatorShare)
            .unwrap_or_default()
            .into();

        let committee_members = self.committee_info.get(&epoch).unwrap().members;
        // todo: pick committee size from genesis.toml or from actual committee?
        let committee_size = committee_members.len();

        for node in committee_members {
            let node_owner = self.node_info.get(&node).unwrap().owner;
            let validator_rewards = (&total_emissions * &validator_share / &percentage_divisor)
                / &committee_size.into();
            self.mint_and_transfer_flk(validator_rewards, node_owner);
        }

        let protocol_share: HpUfloat<18> = self
            .parameters
            .get(&ProtocolParams::ProtocolShare)
            .unwrap_or_default()
            .into();

        let protocol_owner = match self.metadata.get(&Metadata::ProtocolFundAddress) {
            Some(Value::AccountPublicKey(owner)) => owner,
            _ => panic!("ProtocolFundAddress is added at Genesis and should exist"),
        };

        self.mint_and_transfer_flk(
            &total_emissions * (&protocol_share / &percentage_divisor),
            protocol_owner,
        );
    }

    fn calculate_node_revenue(&self, node: &NodePublicKey) -> HpUfloat<6> {
        let served = self.current_epoch_served.get(node).unwrap_or_default();
        served.iter().enumerate().fold(
            HpUfloat::<6>::zero(),
            |mut stables_revenue, (commodity_type, &quantity)| {
                if let Some(price) = CommodityTypes::from_u8(commodity_type as u8)
                    .and_then(|commodity| self.commodity_prices.get(&commodity))
                {
                    let big_quantity: HpUfloat<6> = quantity.into();
                    stables_revenue += price * big_quantity;
                }

                stables_revenue
            },
        )
    }

    fn calculate_emissions(&self) -> (HpUfloat<18>, HpUfloat<18>) {
        let percentage_divisor = HpUfloat::<18>::from(100_u64);
        let inflation_percent: HpUfloat<18> = self
            .parameters
            .get(&ProtocolParams::MaxInflation)
            .unwrap_or(0)
            .into();
        let inflation = &inflation_percent / &percentage_divisor;
        let max_boost: HpUfloat<18> = self
            .parameters
            .get(&ProtocolParams::MaxBoost)
            .unwrap_or(1)
            .into();

        let supply_at_year_start = match self.metadata.get(&Metadata::SupplyYearStart) {
            Some(Value::HpUfloat(supply)) => supply,
            _ => panic!("SupplyYearStart is set genesis and should never be empty"),
        };

        let max_emissions: HpUfloat<18> = (&inflation * &supply_at_year_start) / &365.0.into();

        let min_emissions = &max_emissions / &max_boost;

        (max_emissions, min_emissions)
    }

    fn mint_and_transfer_stables(&self, amount: HpUfloat<6>, owner: EthAddress) {
        let mut account = self.account_info.get(&owner).unwrap_or_default();

        account.stables_balance += amount;
        self.account_info.set(owner, account);
    }

    fn mint_and_transfer_flk(&self, amount: HpUfloat<18>, owner: EthAddress) {
        let mut account = self.account_info.get(&owner).unwrap_or_default();
        account.flk_balance += amount.clone();

        self.account_info.set(owner, account);

        let mut current_supply = match self.metadata.get(&Metadata::TotalSupply) {
            Some(Value::HpUfloat(supply)) => supply,
            _ => panic!("TotalSupply is set genesis and should never be empty"),
        };

        current_supply += amount;
        self.metadata.set(
            Metadata::TotalSupply,
            Value::HpUfloat(current_supply.clone()),
        );

        let current_epoch = match self.metadata.get(&Metadata::Epoch) {
            Some(Value::Epoch(epoch)) => epoch,
            _ => 0,
        };

        if (current_epoch + 1) % 365_u64 == 0_u64 {
            self.metadata
                .set(Metadata::SupplyYearStart, Value::HpUfloat(current_supply));
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
    fn get_boost(&self, locked_until: u64, current_epoch: &Epoch) -> HpUfloat<18> {
        let max_boost: HpUfloat<18> = self
            .parameters
            .get(&ProtocolParams::MaxBoost)
            .unwrap_or(1)
            .into();
        let max_lock_time: HpUfloat<18> = self
            .parameters
            .get(&ProtocolParams::MaxLockTime)
            .unwrap_or(1)
            .into();
        let min_boost = HpUfloat::from(1_u64);
        let locking_period: HpUfloat<18> = (locked_until.saturating_sub(*current_epoch)).into();
        let boost = &min_boost + (&max_boost - &min_boost) * (&locking_period / &max_lock_time);
        HpUfloat::<18>::min(&max_boost, &boost).to_owned()
    }

    fn choose_new_committee(&self) -> Vec<NodePublicKey> {
        // Todo: function not done
        // we need true randomness here, for now we will return the same committee as before to be
        // able to run tests
        let epoch = match self.metadata.get(&Metadata::Epoch) {
            Some(Value::Epoch(epoch)) => epoch,
            _ => 0,
        };
        self.committee_info.get(&epoch).unwrap_or_default().members
    }

    /// This function takes in the Transaction and verifies the Signature matches the Sender. It
    /// also checks the nonce of the sender and makes sure it is equal to the account nonce + 1,
    /// to prevent replay attacks and enforce ordering
    pub fn verify_transaction(&self, txn: &UpdateRequest) -> Result<(), ExecutionError> {
        // Check nonce
        match txn.sender {
            TransactionSender::Node(node) => match self.node_info.get(&node) {
                Some(node_info) => {
                    if txn.payload.nonce != node_info.nonce + 1 {
                        return Err(ExecutionError::InvalidNonce);
                    }
                },
                None => return Err(ExecutionError::NodeDoesNotExist),
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
        match txn.sender {
            TransactionSender::Node(public_key) => match txn.signature {
                TransactionSignature::Node(signature) => {
                    if public_key.verify(&signature, &digest) {
                        Ok(())
                    } else {
                        Err(ExecutionError::InvalidSignature)
                    }
                },
                TransactionSignature::AccountOwner(_) => Err(ExecutionError::InvalidSignature),
            },
            TransactionSender::AccountOwner(eth_address) => match txn.signature {
                TransactionSignature::AccountOwner(signature) => {
                    if eth_address.verify(&signature, &digest) {
                        Ok(())
                    } else {
                        Err(ExecutionError::InvalidSignature)
                    }
                },
                TransactionSignature::Node(_) => Err(ExecutionError::InvalidSignature),
            },
        }
    }

    /// Takes in a zk Proof Of Delivery and returns true if valid
    fn verify_proof_of_delivery(
        &self,
        _client: &EthAddress,
        _provider: &NodePublicKey,
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

    /// Called internally at the end of every transaction to increment the senders nonce.
    /// This happens even if the transaction reverts unless it reverts do to an invalid signature or
    /// nonce This function reverts if the sender does not exist so should be created before
    /// calling this function
    fn increment_nonce(&self, sender: TransactionSender) {
        match sender {
            TransactionSender::Node(node) => {
                let mut node_info = self.node_info.get(&node).unwrap();
                node_info.nonce += 1;
                self.node_info.set(node, node_info);
            },
            TransactionSender::AccountOwner(account) => {
                let mut account_info = self.account_info.get(&account).unwrap();
                account_info.nonce += 1;
                self.account_info.set(account, account_info);
            },
        }
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
    // Useful for transaction that nodes can call but an account owner cant
    // Does not panic
    fn only_node(&self, sender: TransactionSender) -> Result<NodePublicKey, TransactionResponse> {
        match sender {
            TransactionSender::Node(node) => Ok(node),
            _ => Err(TransactionResponse::Revert(ExecutionError::OnlyNode)),
        }
    }
}
