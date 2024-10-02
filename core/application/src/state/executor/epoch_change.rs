use std::cmp::Ordering;
use std::collections::HashMap;

use fleek_blake3::Hasher;
use fleek_crypto::{NodePublicKey, TransactionSender};
use hp_fixed::unsigned::HpUfixed;
use lightning_interfaces::types::{
    Committee,
    CommitteeSelectionBeaconCommit,
    CommitteeSelectionBeaconPhase,
    CommitteeSelectionBeaconReveal,
    Epoch,
    ExecutionData,
    ExecutionError,
    Metadata,
    NodeIndex,
    NodeInfo,
    Participation,
    ProtocolParamKey,
    ProtocolParamValue,
    TransactionResponse,
    Value,
};
use lightning_reputation::statistics;
use lightning_reputation::types::WeightedReputationMeasurements;
use rand::prelude::SliceRandom;
use rand::rngs::StdRng;
use rand::SeedableRng;
use sha3::{Digest, Sha3_256};

use super::{StateExecutor, BIG_HUNDRED, DEFAULT_REP_QUANTILE, MINIMUM_UPTIME, REP_EWMA_WEIGHT};
use crate::state::context::{Backend, TableRef};

impl<B: Backend> StateExecutor<B> {
    /// Handle a change epoch transaction.
    ///
    /// This does not execute the epoch change, it just signals that the epoch change process is
    /// starting, with a committee selection process happening next.
    pub(crate) fn change_epoch(
        &self,
        sender: TransactionSender,
        epoch: Epoch,
    ) -> TransactionResponse {
        // Only Nodes can call this function
        let index = match self.only_node(sender) {
            Ok(account) => account,
            Err(e) => return e,
        };

        // Check that the epoch is current.
        let current_epoch = match self.metadata.get(&Metadata::Epoch) {
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

        // Get the current committee info.
        let mut current_committee = self.committee_info.get(&current_epoch).unwrap_or_default();

        // If sender is not on the current committee revert early, or if they have already signaled;
        if !current_committee.members.contains(&index) {
            return TransactionResponse::Revert(ExecutionError::NotCommitteeMember);
        } else if current_committee.ready_to_change.contains(&index) {
            return TransactionResponse::Revert(ExecutionError::AlreadySignaled);
        }
        current_committee.ready_to_change.push(index);

        // If more than 2/3rds of the committee have signaled, start the committee selection
        // process.
        if current_committee.ready_to_change.len() > (2 * current_committee.members.len() / 3) {
            let start_block = self.get_last_block_number() + 1;
            let end_block = start_block + self.get_commit_phase_duration();

            // Store the start and end block number for the committee selection commit phase.
            tracing::debug!(
                "transitioning to committee selection beacon commit phase because epoch change is starting (start: {}, end: {})",
                start_block,
                end_block
            );
            self.metadata.set(
                Metadata::CommitteeSelectionBeaconPhase,
                Value::CommitteeSelectionBeaconPhase(CommitteeSelectionBeaconPhase::Commit((
                    start_block,
                    end_block,
                ))),
            );

            // Return success.
            TransactionResponse::Success(ExecutionData::None)
        } else {
            self.committee_info.set(current_epoch, current_committee);
            TransactionResponse::Success(ExecutionData::None)
        }
    }

    /// Handle a committee selection beacon commit transaction.
    ///
    /// This transaction is submitted by a node when it wants to commit to a committee selection
    /// random beacon.
    pub fn committee_selection_beacon_commit(
        &self,
        sender: TransactionSender,
        commit: CommitteeSelectionBeaconCommit,
    ) -> TransactionResponse {
        // Check that a node is sending the transaction, and get the node's index.
        let node_index = match self.only_node(sender) {
            Ok(account) => account,
            Err(e) => return e,
        };

        // Log the received commit.
        tracing::debug!(
            "received committee selection beacon commit transaction from node {}: {:?}",
            node_index,
            commit
        );

        // Check that the node has sufficient stake.
        let node = match self.node_info.get(&node_index) {
            Some(node) => node,
            None => return TransactionResponse::Revert(ExecutionError::NodeDoesNotExist),
        };
        // TODO(snormore): What should the minimum stake be? Should it be a protocol parameter?
        if node.stake.staked == HpUfixed::zero() {
            // TODO(snormore): Remove most of these logging statements when the beacon/listener is
            // able to detect and log it's own failed/reverted transactions.
            tracing::debug!("node {} has insufficient stake", node_index);
            return TransactionResponse::Revert(ExecutionError::InsufficientStake);
        }

        // TODO(snormore): Revert if the node was a non-revealing node in the previous round.

        // Get the current block number.
        let current_block = self.get_last_block_number() + 1;

        // Check that the commit phase has started.
        let commit_phase = match self.metadata.get(&Metadata::CommitteeSelectionBeaconPhase) {
            Some(Value::CommitteeSelectionBeaconPhase(CommitteeSelectionBeaconPhase::Commit(
                (start, end),
            ))) => Some((start, end)),
            _ => None,
        };
        if commit_phase.is_none() {
            tracing::debug!(
                "commit phase not started (current block: {})",
                current_block
            );
            return TransactionResponse::Revert(
                ExecutionError::CommitteeSelectionBeaconCommitPhaseNotStarted,
            );
        }

        // Check that the current block number is within the commit phase range.
        if current_block < commit_phase.unwrap().0 || current_block > commit_phase.unwrap().1 {
            tracing::debug!("not in commit phase (current block: {})", current_block);
            return TransactionResponse::Revert(
                ExecutionError::CommitteeSelectionBeaconNotInCommitPhase,
            );
        }

        // Check that the node has not already committed.
        if self.committee_selection_beacon.get(&node_index).is_some() {
            tracing::debug!("node {} already committed", node_index);
            return TransactionResponse::Revert(ExecutionError::CommitteeSelectionAlreadyCommitted);
        }

        // Save the commit beacon.
        tracing::debug!(
            "saving committee selection beacon commit for node {}: {:?}",
            node_index,
            commit
        );
        self.committee_selection_beacon
            .set(node_index, (commit, None));

        // If we are on the last block of the commit phase, restart a new commit phase.
        if current_block == commit_phase.unwrap().1 {
            // Clean up beacon state.
            self.committee_selection_beacon.clear();

            // Reset and start a new commit phase.
            let commit_start = current_block + 1;
            let commit_end = commit_start + self.get_commit_phase_duration();
            tracing::debug!(
                "transitioning to new committee selection beacon commit phase because it is the last block of the current commit phase (start: {}, end: {})",
                commit_start,
                commit_end
            );
            self.metadata.set(
                Metadata::CommitteeSelectionBeaconPhase,
                Value::CommitteeSelectionBeaconPhase(CommitteeSelectionBeaconPhase::Commit((
                    commit_start,
                    commit_end,
                ))),
            );
        }

        // Return success.
        TransactionResponse::Success(ExecutionData::None)
    }

    /// Handle a committee selection beacon reveal transaction.
    ///
    /// This transaction is submitted by a node when it wants to reveal a committee selection
    /// random beacon.
    ///
    /// If all nodes that committed have revealed, the epoch change is executed.
    pub fn committee_selection_beacon_reveal(
        &self,
        sender: TransactionSender,
        reveal: CommitteeSelectionBeaconReveal,
    ) -> TransactionResponse {
        // Check that a node is sending the transaction, and get the node's index.
        let node_index = match self.only_node(sender) {
            Ok(account) => account,
            Err(e) => return e,
        };

        // Check that the node has sufficient stake.
        // TODO(snormore): What should the minimum stake be?
        let node = match self.node_info.get(&node_index) {
            Some(node) => node,
            None => return TransactionResponse::Revert(ExecutionError::NodeDoesNotExist),
        };
        if node.stake.staked == HpUfixed::zero() {
            return TransactionResponse::Revert(ExecutionError::InsufficientStake);
        }

        // Get the current block number.
        let current_block = self.get_last_block_number() + 1;

        // Check that the reveal phase has started.
        let reveal_phase = match self.metadata.get(&Metadata::CommitteeSelectionBeaconPhase) {
            Some(Value::CommitteeSelectionBeaconPhase(CommitteeSelectionBeaconPhase::Reveal(
                (start, end),
            ))) => Some((start, end)),
            _ => None,
        };
        if reveal_phase.is_none() {
            return TransactionResponse::Revert(
                ExecutionError::CommitteeSelectionBeaconRevealPhaseNotStarted,
            );
        }

        // Check that the current block number is within the reveal phase range.
        if current_block < reveal_phase.unwrap().0 || current_block > reveal_phase.unwrap().1 {
            return TransactionResponse::Revert(
                ExecutionError::CommitteeSelectionBeaconNotInRevealPhase,
            );
        }

        // Check that the node has already committed.
        let existing_beacon = self.committee_selection_beacon.get(&node_index);
        if existing_beacon.is_none() {
            return TransactionResponse::Revert(
                ExecutionError::CommitteeSelectionBeaconNotCommitted,
            );
        }
        let (existing_commit, existing_reveal) = existing_beacon.unwrap();

        // Check that the node has not already revealed.
        if existing_reveal.is_some() {
            return TransactionResponse::Revert(ExecutionError::CommitteeSelectionAlreadyRevealed);
        }

        // Check that the reveal is valid.
        let reveal_digest: [u8; 32] = Sha3_256::digest(reveal).into();
        if existing_commit != reveal_digest {
            return TransactionResponse::Revert(
                ExecutionError::CommitteeSelectionBeaconInvalidReveal,
            );
        }

        // Save the reveal beacon.
        self.committee_selection_beacon
            .set(node_index, (existing_commit, Some(reveal)));

        // Get the current epoch.
        let epoch = self.get_epoch();

        // If all nodes that committed have revealed, execute the epoch change.
        let revealed_beacons = self
            .committee_selection_beacon
            .as_map()
            .into_iter()
            .filter(|(_, (_, reveal))| reveal.is_some())
            .collect::<HashMap<_, _>>();
        let committee = match self.committee_info.get(&epoch) {
            Some(committee) => committee,
            // TODO(snormore): This should never happen here; are we doing the right thing by
            // reverting and returning this specific error?
            None => return TransactionResponse::Revert(ExecutionError::MissingCommittee),
        };
        if committee
            .members
            .iter()
            .all(|member| revealed_beacons.contains_key(member))
        {
            // Reset the committee selection beacon phase metadata to indicate we are no longer in
            // commit or reveal phases, we are transitioning to epoch change execution and pre-epoch
            // change waiting phase.
            tracing::debug!(
                "transitioning to committee selection beacon to epoch change execution phase because all committed nodes have revealed (start: {current_block})"
            );
            self.metadata
                .remove(&Metadata::CommitteeSelectionBeaconPhase);

            // Reset the committee selection beacon.
            // TODO(snormore): Do we need to keep any history in the state?
            self.committee_selection_beacon.clear();

            // Execute the epoch change.
            self.execute_epoch_change(epoch, committee);

            // Return success.
            return TransactionResponse::Success(ExecutionData::EpochChange);
        }

        // Return success.
        TransactionResponse::Success(ExecutionData::None)
    }

    /// Handle a committee selection beacon commit phase timeout transaction.
    ///
    /// This transaction is submitted by a node when it sees that the commit phase has timed out.
    pub fn committee_selection_beacon_commit_phase_timeout(
        &self,
        sender: TransactionSender,
    ) -> TransactionResponse {
        // Check that a node is sending the transaction, and get the node's index.
        let _node_index = match self.only_node(sender) {
            Ok(account) => account,
            Err(e) => return e,
        };

        // Get the current block number.
        let current_block = self.get_last_block_number() + 1;
        tracing::debug!(
            "received committee selection beacon commit phase timeout transaction at block {}",
            current_block
        );

        // Get the commit phase metadata.
        let commit_phase = match self.metadata.get(&Metadata::CommitteeSelectionBeaconPhase) {
            Some(Value::CommitteeSelectionBeaconPhase(CommitteeSelectionBeaconPhase::Commit(
                (start, end),
            ))) => (start, end),
            _ => {
                // Revert if not in commit phase.
                return TransactionResponse::Revert(
                    ExecutionError::CommitteeSelectionBeaconCommitPhaseNotStarted,
                );
            },
        };

        // Revert if commit phase has not timed out.
        if current_block <= commit_phase.1 {
            return TransactionResponse::Revert(
                ExecutionError::CommitteeSelectionBeaconCommitPhaseNotTimedOut,
            );
        }

        // The commit phase has timed out.
        // Check if we have sufficient participation or not.
        let epoch = self.get_epoch();
        let current_committee = self.committee_info.get(&epoch).unwrap_or_default();
        let beacons = self.committee_selection_beacon.as_map();
        let committee_beacons = beacons
            .into_iter()
            .filter(|(node_index, _)| current_committee.members.contains(node_index))
            .collect::<HashMap<_, _>>();

        // Check for sufficient participation; that 2/3 of the existing committee has committed. Any
        // other non-committee nodes can also optionally participate, but it's not required.
        if committee_beacons.len() >= (2 * current_committee.members.len() / 3) {
            // If we have sufficient participation, start the reveal phase.
            let reveal_start = current_block + 1;
            let reveal_end = reveal_start + self.get_reveal_phase_duration();
            tracing::debug!(
                "transitioning to committee selection beacon reveal phase because commit phase timed out with sufficient participation (start: {}, end: {})",
                reveal_start,
                reveal_end
            );
            self.metadata.set(
                Metadata::CommitteeSelectionBeaconPhase,
                Value::CommitteeSelectionBeaconPhase(CommitteeSelectionBeaconPhase::Reveal((
                    reveal_start,
                    reveal_end,
                ))),
            );
        } else {
            // If we don't have sufficient participation, start the commit phase again.
            let commit_start = current_block + 1;
            let commit_end = commit_start + self.get_commit_phase_duration();
            tracing::debug!(
                "transitioning to new committee selection beacon commit phase because commit phase timed out without sufficient participation (start: {}, end: {})",
                commit_start,
                commit_end
            );
            self.metadata.set(
                Metadata::CommitteeSelectionBeaconPhase,
                Value::CommitteeSelectionBeaconPhase(CommitteeSelectionBeaconPhase::Commit((
                    commit_start,
                    commit_end,
                ))),
            );

            // Clear the beacon state.
            self.committee_selection_beacon.clear();
        }

        // Return success.
        TransactionResponse::Success(ExecutionData::None)
    }

    /// Handle a committee selection beacon reveal phase timeout transaction.
    ///
    /// This transaction is submitted by a node when it sees that the reveal phase has timed out.
    pub fn committee_selection_beacon_reveal_phase_timeout(
        &self,
        sender: TransactionSender,
    ) -> TransactionResponse {
        // Check that a node is sending the transaction, and get the node's index.
        let _node_index = match self.only_node(sender) {
            Ok(account) => account,
            Err(e) => return e,
        };

        // Get the current block number.
        let current_block = self.get_last_block_number() + 1;
        tracing::debug!(
            "received committee selection beacon reveal phase timeout transaction at block {}",
            current_block
        );

        // Get the reveal phase metadata.
        let reveal_phase = match self.metadata.get(&Metadata::CommitteeSelectionBeaconPhase) {
            Some(Value::CommitteeSelectionBeaconPhase(CommitteeSelectionBeaconPhase::Reveal(
                (start, end),
            ))) => (start, end),
            _ => {
                // Revert if not in reveal phase.
                return TransactionResponse::Revert(
                    ExecutionError::CommitteeSelectionBeaconRevealPhaseNotStarted,
                );
            },
        };

        // Revert if reveal phase has not timed out.
        if current_block <= reveal_phase.1 {
            return TransactionResponse::Revert(
                ExecutionError::CommitteeSelectionBeaconRevealPhaseNotTimedOut,
            );
        }

        // The reveal phase has timed out.
        // Start the commit phase again.
        let commit_start = current_block + 1;
        let commit_end = commit_start + self.get_commit_phase_duration();
        tracing::debug!(
            "transitioning to new committee selection beacon commit phase because reveal phase timed out (start: {}, end: {})",
            commit_start,
            commit_end
        );
        self.metadata.set(
            Metadata::CommitteeSelectionBeaconPhase,
            Value::CommitteeSelectionBeaconPhase(CommitteeSelectionBeaconPhase::Commit((
                commit_start,
                commit_end,
            ))),
        );

        // Clear the beacon state.
        for digest in self.committee_selection_beacon.keys() {
            self.committee_selection_beacon.remove(&digest);
        }

        // Return success.
        TransactionResponse::Success(ExecutionData::None)
    }

    /********Internal Application Functions******** */
    // These functions should only ever be called in the context of an external transaction function
    // They should never panic and any check that could result in that should be done in the
    // external function that calls it The functions that should call this and the required
    // checks should be documented for each function

    fn get_commit_phase_duration(&self) -> u64 {
        match self
            .parameters
            .get(&ProtocolParamKey::CommitteeSelectionBeaconCommitPhaseDuration)
        {
            Some(ProtocolParamValue::CommitteeSelectionBeaconCommitPhaseDuration(v)) => v,
            None => unreachable!("missing commit phase duration"),
            _ => unreachable!("invalid commit phase duration"),
        }
    }

    fn get_reveal_phase_duration(&self) -> u64 {
        match self
            .parameters
            .get(&ProtocolParamKey::CommitteeSelectionBeaconRevealPhaseDuration)
        {
            Some(ProtocolParamValue::CommitteeSelectionBeaconRevealPhaseDuration(v)) => v,
            None => unreachable!("missing reveal phase duration"),
            _ => unreachable!("invalid reveal phase duration"),
        }
    }

    fn execute_epoch_change(&self, epoch: Epoch, current_committee: Committee) {
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
        self.executed_digests.clear();

        self.committee_info.set(epoch, current_committee);
        // Get new committee
        let new_committee = self.choose_new_committee();

        // increment epoch
        let epoch = epoch + 1;

        // Set the new committee, epoch, and reset sub dag index
        self.committee_info.set(epoch, new_committee);

        // Save new epoch to metadata.
        self.metadata.set(Metadata::Epoch, Value::Epoch(epoch));
    }

    fn choose_new_committee(&self) -> Committee {
        let epoch = match self.metadata.get(&Metadata::Epoch) {
            Some(Value::Epoch(epoch)) => epoch,
            _ => 0,
        };

        let node_registry: Vec<(NodeIndex, NodeInfo)> = self
            .get_node_registry()
            .into_iter()
            .filter(|index| index.1.participation == Participation::True)
            .collect();

        let committee_size = match self.parameters.get(&ProtocolParamKey::CommitteeSize) {
            Some(ProtocolParamValue::CommitteeSize(v)) => v,
            _ => unreachable!(), // set in genesis
        };

        let mut active_nodes: Vec<NodeIndex> = self
            .settle_auction(node_registry)
            .iter()
            .map(|node| node.0)
            .collect();

        let num_of_nodes = active_nodes.len() as u64;
        // if total number of nodes are less than committee size, all nodes are part of committee
        let committee = if committee_size >= num_of_nodes {
            active_nodes.clone()
            //   return node_registry;
        } else {
            let committee_table = self.committee_info.get(&epoch).unwrap();
            // TODO(snormore): Use the aggregated random beacon instead of the
            // `epoch_end_timestamp`. Make sure this is tested, that there are non-committee nodes
            // in the network or else this code path will never get hit.
            let epoch_end = committee_table.epoch_end_timestamp;
            let public_key = {
                if !committee_table.members.is_empty() {
                    let mid_index = committee_table.members.len() / 2;
                    self.node_info
                        .get(&committee_table.members[mid_index])
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
            active_nodes.shuffle(&mut rng);
            active_nodes
                .iter()
                .take(committee_size.try_into().unwrap())
                .copied()
                .collect()
        };

        let epoch_length = match self.parameters.get(&ProtocolParamKey::EpochTime) {
            Some(ProtocolParamValue::EpochTime(v)) => v,
            _ => unreachable!(), // set in genesis
        };

        let epoch_end_timestamp =
            self.committee_info.get(&epoch).unwrap().epoch_end_timestamp + epoch_length as u64;

        Committee {
            ready_to_change: Vec::with_capacity(committee.len()),
            members: committee,
            epoch_end_timestamp,
            active_node_set: active_nodes,
        }
    }

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

        let node_percentage = match self.parameters.get(&ProtocolParamKey::NodeShare) {
            Some(ProtocolParamValue::NodeShare(v)) => HpUfixed::<18>::from(v),
            _ => unreachable!(), // set in genesis
        };

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
        let service_share = match self.parameters.get(&ProtocolParamKey::ServiceBuilderShare) {
            Some(ProtocolParamValue::ServiceBuilderShare(v)) => {
                HpUfixed::<18>::from(v) / &(*BIG_HUNDRED)
            },
            _ => unreachable!(), // set in genesis
        };

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
        let protocol_share = match self.parameters.get(&ProtocolParamKey::ProtocolShare) {
            Some(ProtocolParamValue::ProtocolShare(v)) => HpUfixed::<18>::from(v) / &(*BIG_HUNDRED),
            _ => unreachable!(), // set in genesis
        };

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

        let min_num_measurements = match self.parameters.get(&ProtocolParamKey::MinNumMeasurements)
        {
            Some(ProtocolParamValue::MinNumMeasurements(v)) => v as usize,
            _ => unreachable!(), // set in genesis
        };

        let mut map = HashMap::new();
        for node in self.rep_measurements.keys() {
            if let Some(reported_measurements) = self.rep_measurements.get(&node) {
                if reported_measurements.len() >= min_num_measurements {
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

    fn calculate_emissions(&self) -> HpUfixed<18> {
        let percentage_divisor = HpUfixed::<18>::from(100_u64);

        let inflation_percent = match self.parameters.get(&ProtocolParamKey::MaxInflation) {
            Some(ProtocolParamValue::MaxInflation(v)) => HpUfixed::<18>::from(v),
            _ => HpUfixed::<18>::from(0_u16), // set in genesis
        };

        let epoch_per_year = self.get_epochs_per_year();

        let inflation: HpUfixed<18> =
            (&inflation_percent / &percentage_divisor).convert_precision();

        let supply_at_year_start = match self.metadata.get(&Metadata::SupplyYearStart) {
            Some(Value::HpUfixed(supply)) => supply,
            _ => panic!("SupplyYearStart is set genesis and should never be empty"),
        };

        (&inflation * &supply_at_year_start.convert_precision()) / &epoch_per_year.into()
    }
}
