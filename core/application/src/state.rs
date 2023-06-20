use std::{
    cmp::Ordering,
    collections::{BTreeMap, HashMap},
};

use big_decimal::BigDecimal;
use draco_interfaces::{
    types::{
        AccountInfo, CommodityServed, CommodityTypes, Epoch, ExecutionData, ExecutionError,
        Metadata, NodeInfo, ProofOfConsensus, ProofOfMisbehavior, ProtocolParams,
        ReportedReputationMeasurements, ReputationMeasurements, Service, ServiceId, Staking,
        Tokens, TotalServed, TransactionResponse, UpdateMethod, UpdateRequest, Worker,
    },
    DeliveryAcknowledgment,
};
use draco_reputation::{statistics, WeightedReputationMeasurements};
use fleek_crypto::{
    AccountOwnerPublicKey, ClientPublicKey, NodeNetworkingPublicKey, NodePublicKey,
    TransactionSender,
};
use multiaddr::Multiaddr;
use num_traits::FromPrimitive;
use serde::{Deserialize, Serialize};

use crate::table::{Backend, TableRef};

/// Minimum number of reported measurements that have to be available for a node.
/// If less measurements have been reported, no reputation score will be computed in that epoch.
const MIN_NUM_MEASUREMENTS: usize = 10;

/// Reported measurements are weighted by the reputation score of the reporting node.
/// If there is no reputation score for the reporting node, we use a quantile from the array
/// of all reputation scores.
/// For example, if `DEFAULT_REP_QUANTILE = 0.3`, then we use the reputation score that is higher
/// than 30% of scores and lower than 70% of scores.
const DEFAULT_REP_QUANTILE: f64 = 0.3;

/// The state of the Application
///
/// The functions implemented on this struct are the "Smart Contracts" of the application layer
/// All state changes come from Transactions and start at execute_txn
pub struct State<B: Backend> {
    pub metadata: B::Ref<Metadata, BigDecimal<18>>,
    pub account_info: B::Ref<AccountOwnerPublicKey, AccountInfo>,
    pub client_keys: B::Ref<ClientPublicKey, AccountOwnerPublicKey>,
    pub node_info: B::Ref<NodePublicKey, NodeInfo>,
    pub committee_info: B::Ref<Epoch, Committee>,
    pub services: B::Ref<ServiceId, Service>,
    pub parameters: B::Ref<ProtocolParams, u128>,
    pub rep_measurements: B::Ref<NodePublicKey, Vec<ReportedReputationMeasurements>>,
    pub rep_scores: B::Ref<NodePublicKey, u8>,
    pub current_epoch_served: B::Ref<NodePublicKey, CommodityServed>,
    pub last_epoch_served: B::Ref<NodePublicKey, CommodityServed>,
    pub total_served: B::Ref<Epoch, TotalServed>,
    pub commodity_prices: B::Ref<CommodityTypes, f64>,
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
        if !self.backend.verify_proof_of_delivery(
            &account_owner,
            &sender,
            &commodity,
            &service_id,
            (),
        ) {
            return TransactionResponse::Revert(ExecutionError::InvalidProof);
        }

        let commodity_type = self
            .services
            .get(&service_id)
            .map(|s| s.commodity_type)
            .unwrap();

        let current_epoch = self
            .metadata
            .get(&Metadata::Epoch)
            .unwrap_or_default()
            .into();

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
        total_served.reward_pool += commodity as f64 * commodity_prices;

        self.current_epoch_served.set(sender, commodity_served);
        self.total_served.set(current_epoch, total_served);

        TransactionResponse::Success(ExecutionData::None)
    }

    fn withdraw(
        &self,
        _sender: TransactionSender,
        _reciever: AccountOwnerPublicKey,
        _amount: BigDecimal<18>,
        _token: Tokens,
    ) -> TransactionResponse {
        todo!()
    }

    fn deposit(
        &self,
        sender: TransactionSender,
        proof: ProofOfConsensus,
        amount: BigDecimal<18>,
        token: Tokens,
    ) -> TransactionResponse {
        // This transaction is only callable by AccountOwners and not nodes
        // So revert if the sender is a node public key
        let sender = match self.only_account_owner(sender) {
            Ok(account) => account,
            Err(e) => return e,
        };

        // Verify the proof from the bridge
        if !self.backend.verify_proof_of_consensus(proof) {
            return TransactionResponse::Revert(ExecutionError::InvalidProof);
        }

        let mut account = self.account_info.get(&sender).unwrap_or_default();

        // Check the token bridged and increment that amount
        match token {
            Tokens::FLK => account.flk_balance += amount,
            Tokens::USDC => account.bandwidth_balance += Into::<u128>::into(amount),
        }

        self.account_info.set(sender, account);
        TransactionResponse::Success(ExecutionData::None)
    }

    #[allow(clippy::too_many_arguments)]
    fn stake(
        &self,
        sender: TransactionSender,
        amount: BigDecimal<18>,
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
                        staked_since: self
                            .metadata
                            .get(&Metadata::Epoch)
                            .unwrap_or_default()
                            .into(),
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
        if node.owner != sender {
            return TransactionResponse::Revert(ExecutionError::NotNodeOwner);
        }
        // check if node has stakes to be locked
        if node.stake.staked == BigDecimal::zero() {
            return TransactionResponse::Revert(ExecutionError::InsufficientStakesToLock);
        }

        let epoch = self
            .metadata
            .get(&Metadata::Epoch)
            .unwrap_or_default()
            .into();
        let current_lock = node.stake.stake_locked_until.saturating_sub(epoch);

        let max_lock_time = self
            .parameters
            .get(&ProtocolParams::MaxLockTime)
            .unwrap_or_default() as u64;

        // check if total locking is greater than max locking
        if current_lock + locked_for > max_lock_time {
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
        amount: BigDecimal<18>,
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
        if node.owner != sender {
            return TransactionResponse::Revert(ExecutionError::NotNodeOwner);
        }

        let current_epoch = self
            .metadata
            .get(&Metadata::Epoch)
            .unwrap_or_default()
            .into();

        // Make sure the stakes are not locked
        if node.stake.stake_locked_until > current_epoch {
            return TransactionResponse::Revert(ExecutionError::LockedTokensUnstakeForbidden);
        }

        // Make sure the node has atleast that much staked
        if node.stake.staked < amount {
            return TransactionResponse::Revert(ExecutionError::InsufficientBalance);
        }

        let lock_time = self
            .parameters
            .get(&ProtocolParams::LockTime)
            .unwrap_or_default();

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
        recipient: Option<AccountOwnerPublicKey>,
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
        if node.owner != sender_public_key {
            return TransactionResponse::Revert(ExecutionError::NotNodeOwner);
        }

        let current_epoch = self.metadata.get(&Metadata::Epoch).unwrap_or_default();
        // Make sure the node has locked tokens and that the lock time is passed
        if node.stake.locked == BigDecimal::zero() {
            return TransactionResponse::Revert(ExecutionError::NoLockedTokens);
        }
        if node.stake.locked_until > current_epoch.into() {
            return TransactionResponse::Revert(ExecutionError::TokensLocked);
        }

        // if there is no recipient the owner will recieve the withdrawl
        let recipient = recipient.unwrap_or(sender_public_key);
        let mut reciever = self.account_info.get(&recipient).unwrap_or_default();

        // add the withdrawn tokens to the recipient and reset the nodes locked stake state
        // no need to reset locked_until on the node because that will get adjusted the next time
        // the node unstakes
        reciever.flk_balance += node.stake.locked;
        node.stake.locked = BigDecimal::zero();

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

        let mut current_epoch = self
            .metadata
            .get(&Metadata::Epoch)
            .unwrap_or_default()
            .into();

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

            // calculate the next epoch endstamp
            let epoch_duration = self.parameters.get(&ProtocolParams::EpochTime).unwrap();
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

            self.distribute_rewards();

            self.metadata.set(Metadata::Epoch, current_epoch.into());
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
                if reported_measurements.len() > MIN_NUM_MEASUREMENTS {
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

        new_rep_scores
            .into_iter()
            .for_each(|(node, score)| self.rep_scores.set(node, score));

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
    /// current epoch. The rewards are given in two forms: 1) A "stable" currency that is
    /// proportional to the revenue earned by selling the commodities. 2) "FLK" token that is
    /// proportional to the "stable" currency rewards, but also depends on a "boost" factor.
    ///     FLk emission per unit of stable earned is given by:
    ///
    /// emission = (inflation * supply) / (maxBoost * rewardPool * 365.0)
    fn distribute_rewards(&self) {
        // Todo: function not done
        let epoch = self
            .metadata
            .get(&Metadata::Epoch)
            .unwrap_or_default()
            .into();

        let reward_pool = self
            .total_served
            .get(&epoch)
            .unwrap_or_default()
            .reward_pool;

        // if reward is 0, no commodity under any service was served
        if reward_pool == 0_f64 {
            return;
        }
        let inflation: BigDecimal<18> = (self
            .parameters
            .get(&ProtocolParams::MaxInflation)
            .unwrap_or_default() as f64
            / 100.0)
            .into();
        let max_boost: BigDecimal<18> = self
            .parameters
            .get(&ProtocolParams::MaxBoost)
            .unwrap_or(1)
            .into();

        let supply_at_year_start = self
            .metadata
            .get(&Metadata::SupplyYearStart)
            .unwrap_or_default();

        let total_emissions: BigDecimal<18> =
            (inflation * supply_at_year_start) / (max_boost * 365.0.into());
        // Todo: distribute emission rewards to other stakeholders(validators and foundation
        // address)

        let flk_per_stable_revenue_unit = total_emissions / reward_pool.into();

        let nodes = self.current_epoch_served.keys();
        for node in nodes {
            let served = self.current_epoch_served.get(&node).unwrap_or_default();
            let mut stables_revenue: f64 = 0.0;
            // Iterate over the quantities and their corresponding commodity type
            for (commodity_type, &quantity) in served.iter().enumerate() {
                // Convert the index to CommodityTypes
                if let Some(commodity) = CommodityTypes::from_u8(commodity_type as u8) {
                    // Get the price for the commodity
                    if let Some(price) = self.commodity_prices.get(&commodity) {
                        // Calculate the total price for this quantity of commodity
                        stables_revenue += price * quantity as f64;
                    }
                }
            }
            let big_stables_revenue_6: BigDecimal<6> = BigDecimal::<6>::from(stables_revenue);
            self.mint_and_transfer_stables(big_stables_revenue_6, node);

            // safe to unwrap since all the nodes in current_epoch_served table are in node info
            // this is checked in submit_pod contract/function
            let locked_until = self.node_info.get(&node).unwrap().stake.stake_locked_until;
            let boost: BigDecimal<18> = self.get_boost(locked_until, epoch).into();
            let big_stables_revenue = BigDecimal::<18>::from(stables_revenue);
            let flk_rewards = big_stables_revenue * flk_per_stable_revenue_unit.clone() * boost;
            self.mint_and_transfer_flk(flk_rewards, node);
            self.current_epoch_served.remove(&node);
        }
    }

    fn mint_and_transfer_stables(&self, amount: BigDecimal<6>, node: NodePublicKey) {
        let owner = self.node_info.get(&node).unwrap().owner;
        let mut account = self.account_info.get(&owner).unwrap_or_default();

        account.stables_balance += amount;
        self.account_info.set(owner, account);
    }

    fn mint_and_transfer_flk(&self, amount: BigDecimal<18>, node: NodePublicKey) {
        let owner = self.node_info.get(&node).unwrap().owner;
        let mut account = self.account_info.get(&owner).unwrap_or_default();
        account.flk_balance += amount.clone();

        self.account_info.set(owner, account);

        let mut current_supply = self
            .metadata
            .get(&Metadata::TotalSupply)
            .unwrap_or_default();

        current_supply += amount;
        self.metadata
            .set(Metadata::TotalSupply, current_supply.clone());

        let current_epoch: u128 = self
            .metadata
            .get(&Metadata::Epoch)
            .unwrap_or_default()
            .into();

        if (current_epoch + 1) % 365_u128 == 0_u128 {
            self.metadata.set(Metadata::SupplyYearStart, current_supply);
        }
    }

    /// The `get_boost` method calculates a reward boost factor based on the locked staking period.
    ///
    /// This function follows a power-law growth model, where the boost increases with the power of
    /// the elapsed time. The locked period is determined by the difference between the current
    /// epoch and the 'locked_until' parameter, signifying the epoch until which the stake is
    /// locked.
    ///
    /// As the stake nears its expiry, the reward boost decreases according to the function.
    ///
    /// The calculated boost value is capped at `maxBoost`, a parameter set by the protocol, to
    /// prevent excessive boosts. The boost formula is as follows:
    ///
    /// boost(t) = min(maxBoost, minBoost + (maxBoost - minBoost) * (t/max_T)^n)
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
    fn get_boost(&self, locked_until: u64, current_epoch: u64) -> f64 {
        let max_boost = self.parameters.get(&ProtocolParams::MaxBoost).unwrap_or(1);
        let max_lock_time = self
            .parameters
            .get(&ProtocolParams::MaxLockTime)
            .unwrap_or_default() as f64;
        let locking_period = locked_until.saturating_sub(current_epoch) as f64;
        let boost: f64 =
            1.0 + (max_boost as f64 - 1.0) * (locking_period / max_lock_time).powf(1.2);
        f64::min(max_boost as f64, boost)
    }

    fn choose_new_committee(&self) -> Vec<NodePublicKey> {
        // Todo: function not done
        // we need true randomness here, for now we will return the same committee as before to be
        // able to run tests
        let epoch = self
            .metadata
            .get(&Metadata::Epoch)
            .unwrap_or_default()
            .into();
        self.committee_info.get(&epoch).unwrap_or_default().members
    }

    /// Called internally at the end of every transaction to increment the senders nonce.
    /// This happens even if the transaction reverts unless it reverts do to an invalid signature or
    /// nonce This function reverts if the sender does not exist so should be created before
    /// calling this function
    fn increment_nonce(&self, sender: TransactionSender) {
        match sender {
            TransactionSender::Node(node) => {
                self.node_info.get(&node).unwrap().nonce += 1;
            },
            TransactionSender::AccountOwner(account) => {
                self.account_info.get(&account).unwrap().nonce += 1;
            },
        }
    }
    // Useful for transaction that nodes cannot call but an account owner can
    // Does not panic
    fn only_account_owner(
        &self,
        sender: TransactionSender,
    ) -> Result<AccountOwnerPublicKey, TransactionResponse> {
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
