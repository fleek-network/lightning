use std::{
    cmp::Ordering,
    collections::{BTreeMap, HashMap},
};

// use bigdecimal::{self, BigDecimal};
use draco_interfaces::{
    types::{
        AccountInfo, CommodityServed, CommodityTypes, Epoch, ExecutionData, ExecutionError,
        Metadata, NodeInfo, ProofOfConsensus, ProofOfMisbehavior, ProtocolParams,
        ReportedReputationMeasurements, ReputationMeasurements, Service, ServiceId, Staking,
        Tokens, TotalServed, TransactionResponse, UpdateMethod, UpdateRequest, Worker,
    },
    DeliveryAcknowledgment,
};
use fleek_crypto::{
    AccountOwnerPublicKey, ClientPublicKey, NodeNetworkingPublicKey, NodePublicKey,
    TransactionSender,
};
use multiaddr::Multiaddr;
use num_bigint::{BigUint, ToBigUint};
use num_traits::{FromPrimitive, One, ToPrimitive, Zero};
use serde::{Deserialize, Serialize};

use crate::{
    big_decimal::BigDecimal,
    table::{Backend, TableRef},
};

/// Minimum number of reported measurements that have to be available for a node.
/// If less measurements have been reported, no reputation score will be computed in that epoch.
const MIN_NUM_MEASUREMENTS: usize = 10;

/// The state of the Application
///
/// The functions implemented on this struct are the "Smart Contracts" of the application layer
/// All state changes come from Transactions and start at execute_txn
pub struct State<B: Backend> {
    pub metadata: B::Ref<Metadata, BigUint>,
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
            .to_u64()
            .unwrap();

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
        _amount: BigUint,
        _token: Tokens,
    ) -> TransactionResponse {
        todo!()
    }

    fn deposit(
        &self,
        sender: TransactionSender,
        proof: ProofOfConsensus,
        amount: BigUint,
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
            Tokens::USDC => account.bandwidth_balance += amount.to_u128().unwrap(),
        }

        self.account_info.set(sender, account);
        TransactionResponse::Success(ExecutionData::None)
    }

    #[allow(clippy::too_many_arguments)]
    fn stake(
        &self,
        sender: TransactionSender,
        amount: BigUint,
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
                            .to_u64()
                            .unwrap(),
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
        if node.stake.staked == Zero::zero() {
            return TransactionResponse::Revert(ExecutionError::InsufficientStakesToLock);
        }

        let epoch = self
            .metadata
            .get(&Metadata::Epoch)
            .unwrap_or_default()
            .to_u64()
            .unwrap();
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
        amount: BigUint,
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
            .to_u64()
            .unwrap();

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
        if node.stake.locked == Zero::zero() {
            return TransactionResponse::Revert(ExecutionError::NoLockedTokens);
        }
        if node.stake.locked_until > current_epoch.to_u64().unwrap() {
            return TransactionResponse::Revert(ExecutionError::TokensLocked);
        }

        // if there is no recipient the owner will recieve the withdrawl
        let recipient = recipient.unwrap_or(sender_public_key);
        let mut reciever = self.account_info.get(&recipient).unwrap_or_default();

        // add the withdrawn tokens to the recipient and reset the nodes locked stake state
        // no need to reset locked_until on the node because that will get adjusted the next time
        // the node unstakes
        reciever.flk_balance += node.stake.locked;
        node.stake.locked = Zero::zero();

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
            .to_u64()
            .unwrap();

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
            // self.calculate_reputation_scores();

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

            // todo: move contents of current_epoch_served to last_epoch_served
            // reset the current_epoch_served
            self.distribute_rewards();

            self.metadata
                .set(Metadata::Epoch, current_epoch.to_biguint().unwrap());
            TransactionResponse::Success(ExecutionData::EpochChange)
        } else {
            self.committee_info.set(current_epoch, current_committee);
            TransactionResponse::Success(ExecutionData::None)
        }
    }

    #[allow(dead_code)]
    fn calculate_reputation_scores(&self) {
        let mut map = HashMap::new();
        for node in self.rep_measurements.keys() {
            if let Some(reported_measurements) = self.rep_measurements.get(&node) {
                if reported_measurements.len() > MIN_NUM_MEASUREMENTS {
                    // Only compute reputation score for node if enough measurements have been
                    // reported
                    map.insert(node, reported_measurements);
                }
            }
        }
        let rep_scores = draco_reputation::calculate_reputation_scores(map);
        rep_scores
            .into_iter()
            .for_each(|(node, score)| self.rep_scores.set(node, score));
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

    // This function should be called during signal_epoch_change.
    fn distribute_rewards(&self) {
        // Todo: function not done
        let epoch = self
            .metadata
            .get(&Metadata::Epoch)
            .unwrap_or_default()
            .to_u64()
            .unwrap();

        let reward_pool = self
            .total_served
            .get(&epoch)
            .unwrap_or_default()
            .reward_pool;

        // check if there is any service, 0 reward pools mean no service
        if reward_pool == 0_f64 {
            return;
        }
        let inflation = self
            .parameters
            .get(&ProtocolParams::MaxInflation)
            .unwrap_or_default();
        let max_boost = self.parameters.get(&ProtocolParams::MaxBoost).unwrap_or(1);

        let supply_at_year_start = self
            .metadata
            .get(&Metadata::SupplyYearStart)
            .unwrap_or_default();

        // todo: refactor this into our own struct of BigDecimal
        let emission_per_revenue_unit = inflation as f64 / (max_boost as f64 * reward_pool);
        let bd_emission = BigDecimal::from_f64(emission_per_revenue_unit, 18);

        let flk_per_stable_revenue_unit = bd_emission.value * supply_at_year_start;

        // Todo: iterate over last_epoch_served table
        // distribute inflation rewards per unit of usdc earned
        // formula for rewards distribution per node
        // distribute usdc rewards per unit of commodity served
        // usdc rewards are just simply unit served per commodity * commodity price
        let nodes = self.current_epoch_served.keys();
        for node in nodes {
            let served = self.current_epoch_served.get(&node).unwrap_or_default();
            let mut stable_revenue: f64 = 0.0;
            // Iterate over the quantities and their corresponding commodity type
            for (commodity_type, &quantity) in served.iter().enumerate() {
                // Convert the index to CommodityTypes
                if let Some(commodity) = CommodityTypes::from_u8(commodity_type as u8) {
                    // Get the price for the commodity
                    if let Some(price) = self.commodity_prices.get(&commodity) {
                        // Calculate the total price for this quantity of commodity
                        stable_revenue += price * quantity as f64;
                    }
                }
            }
            let big_stable_revenue = BigDecimal::from_f64(stable_revenue, 18);
            self.mint_and_transfer(big_stable_revenue.value.clone(), node, Tokens::USDC);
            self.mint_and_transfer(
                big_stable_revenue.value.clone() * flk_per_stable_revenue_unit.clone(),
                node,
                Tokens::FLK,
            )
        }
    }

    fn mint_and_transfer(&self, amount: BigUint, node: NodePublicKey, token: Tokens) {
        let owner = self.node_info.get(&node).unwrap().owner;
        let mut account = self.account_info.get(&owner).unwrap();

        match token {
            Tokens::USDC => account.stables_balance += amount,
            Tokens::FLK => {
                account.flk_balance += amount.clone();

                self.account_info.set(owner, account);

                let mut current_supply = self
                    .metadata
                    .get(&Metadata::TotalSupply)
                    .unwrap_or_default();

                current_supply += amount;
                self.metadata
                    .set(Metadata::TotalSupply, current_supply.clone());

                let current_epoch = self.metadata.get(&Metadata::Epoch).unwrap_or_default();

                let days_in_year = BigUint::from(365u32);
                if current_epoch.modpow(&BigUint::one(), &days_in_year) == BigUint::zero() {
                    let mut supply_start_year = self
                        .metadata
                        .get(&Metadata::SupplyYearStart)
                        .unwrap_or_default();

                    supply_start_year += current_supply;
                    self.metadata
                        .set(Metadata::SupplyYearStart, supply_start_year);
                }
            },
        }
    }

    fn choose_new_committee(&self) -> Vec<NodePublicKey> {
        // Todo: function not done
        // we need true randomness here, for now we will return the same committee as before to be
        // able to run tests
        let epoch = self
            .metadata
            .get(&Metadata::Epoch)
            .unwrap_or_default()
            .to_u64()
            .unwrap();
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
