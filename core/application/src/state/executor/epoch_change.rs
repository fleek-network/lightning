use std::cmp::Ordering;
use std::collections::HashMap;

use fleek_crypto::TransactionSender;
use fxhash::FxHashMap;
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
    NodeRegistryChange,
    NodeRegistryChangeSlashReason,
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
        let index = match self.only_node_with_sufficient_stake_and_participating(sender) {
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

        // Save the updated committee info and ready-to-change set.
        self.committee_info
            .set(current_epoch, current_committee.clone());

        // If more than 2/3rds of the committee have signaled, start the committee selection
        // process.
        // Note that we do NOT want to execute this code after 2/3+1 nodes have signaled, since
        // we only want to do it once to start the process.
        if current_committee.ready_to_change.len() == (2 * current_committee.members.len() / 3) + 1
        {
            let start_block = self.get_block_number() + 1;
            let end_block = start_block + self.get_commit_phase_duration();

            tracing::info!(
                "transitioning to committee selection beacon commit phase because epoch change is starting (epoch: {}, start: {}, end: {})",
                epoch,
                start_block,
                end_block
            );

            // Store the start and end block number for the committee selection commit phase.
            self.metadata.set(
                Metadata::CommitteeSelectionBeaconPhase,
                Value::CommitteeSelectionBeaconPhase(CommitteeSelectionBeaconPhase::Commit((
                    start_block,
                    end_block,
                ))),
            );

            // Start the committee selection beacon round at 0.
            self.set_committee_selection_beacon_round(0);

            // Reset the epoch era.
            self.metadata.set(Metadata::EpochEra, Value::EpochEra(0));

            // Return success.
            TransactionResponse::Success(ExecutionData::None)
        } else {
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
        let node_index = match self.only_node_with_sufficient_stake(sender) {
            Ok(account) => account,
            Err(e) => return e,
        };

        // Check that the node is in the active set.
        let epoch = self.get_epoch();
        let active_nodes = self
            .committee_info
            .get(&epoch)
            .unwrap_or_default()
            .active_node_set;
        if !active_nodes.contains(&node_index) {
            return TransactionResponse::Revert(
                ExecutionError::CommitteeSelectionBeaconNodeNotActive,
            );
        }

        // Revert if the node was a non-revealing node in the previous round.
        if self
            .committee_selection_beacon_non_revealing_node
            .get(&node_index)
            .is_some()
        {
            return TransactionResponse::Revert(
                ExecutionError::CommitteeSelectionBeaconNonRevealingNode,
            );
        }

        // Get the current block number.
        let current_block = self.get_block_number() + 1;

        // Check that the commit phase has started.
        let commit_phase = match self.metadata.get(&Metadata::CommitteeSelectionBeaconPhase) {
            Some(Value::CommitteeSelectionBeaconPhase(CommitteeSelectionBeaconPhase::Commit(
                (start, end),
            ))) => Some((start, end)),
            _ => None,
        };
        if commit_phase.is_none() {
            return TransactionResponse::Revert(
                ExecutionError::CommitteeSelectionBeaconCommitPhaseNotStarted,
            );
        }

        // Check that the current block number is within the commit phase range.
        if current_block < commit_phase.unwrap().0 || current_block > commit_phase.unwrap().1 {
            return TransactionResponse::Revert(
                ExecutionError::CommitteeSelectionBeaconNotInCommitPhase,
            );
        }

        // Check that the node has not already committed.
        if self.committee_selection_beacon.get(&node_index).is_some() {
            return TransactionResponse::Revert(
                ExecutionError::CommitteeSelectionBeaconAlreadyCommitted,
            );
        }

        // Save the commit beacon.
        self.committee_selection_beacon
            .set(node_index, (commit, None));

        // If all active nodes have committed, we can transition to the reveal phase early before
        // timeout.
        let beacons = self.committee_selection_beacon.as_map();
        if beacons.len() == active_nodes.len()
            && active_nodes
                .iter()
                .all(|node_index| beacons.contains_key(node_index))
        {
            let reveal_start = current_block + 1;
            let reveal_end = reveal_start + self.get_reveal_phase_duration();
            tracing::info!(
                "transitioning to committee selection beacon reveal phase because all active nodes have committed (epoch: {}, start: {}, end: {})",
                self.get_epoch(),
                reveal_start,
                reveal_end
            );
            self.set_committee_selection_beacon_reveal_phase(reveal_start, reveal_end);
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
        let node_index = match self.only_node_with_sufficient_stake(sender) {
            Ok(account) => account,
            Err(e) => return e,
        };

        // NOTE: We don't check for the node being in the active set here because we checked for it
        // in the commit transaction, and if the node has since been removed from the active set,
        // but still has sufficient stake to be a valid node, we give it the opportunity to reveal
        // after committing to avoid becoming a non-revealing node and getting slashed.

        // Get the current block number.
        let current_block = self.get_block_number() + 1;

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
            return TransactionResponse::Revert(
                ExecutionError::CommitteeSelectionBeaconAlreadyRevealed,
            );
        }

        // Get the current epoch.
        let epoch = self.get_epoch();

        // Check that the reveal is valid.
        let round = self.get_committee_selection_beacon_round();
        if round.is_none() {
            return TransactionResponse::Revert(
                ExecutionError::CommitteeSelectionBeaconRoundNotFound,
            );
        }
        let commit = CommitteeSelectionBeaconCommit::build(epoch, round.unwrap(), reveal);
        if existing_commit != commit {
            return TransactionResponse::Revert(
                ExecutionError::CommitteeSelectionBeaconInvalidReveal,
            );
        }

        // Save the reveal beacon.
        self.committee_selection_beacon
            .set(node_index, (existing_commit, Some(reveal)));

        // If all nodes that committed have revealed, execute the epoch change.
        let committee = self.committee_info.get(&epoch).unwrap_or_default();
        let beacons = self.committee_selection_beacon.as_map();
        if beacons.iter().all(|(_, (_, reveal))| reveal.is_some()) {
            tracing::info!(
                "transitioning to committee selection beacon to epoch change execution phase because all committed nodes have revealed (epoch: {}, start: {})",
                epoch,
                current_block,
            );

            // Reset the committee selection beacon phase metadata to indicate we are no longer in
            // commit or reveal phases, we are transitioning to epoch change execution and pre-epoch
            // change waiting phase.
            self.clear_committee_selection_beacon_phase();

            // Remove the committee selection beacon round.
            self.clear_committee_selection_beacon_round();

            // Reset the committee selection beacon.
            self.clear_committee_selection_beacons();

            // Execute the epoch change.
            self.execute_epoch_change(epoch, committee, beacons);

            // Return success.
            return TransactionResponse::Success(ExecutionData::EpochChange);
        }

        let commit_count = beacons.len();
        let reveal_count = beacons
            .iter()
            .filter(|(_, (_, reveal))| reveal.is_some())
            .count();
        let (phase_start, phase_end) = reveal_phase.unwrap();
        tracing::debug!(
            "waiting for remaining nodes to reveal (epoch: {}, start: {}, end: {}, committed: {}, revealed: {})",
            epoch,
            phase_start,
            phase_end,
            commit_count,
            reveal_count,
        );

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
        let _node_index = match self.only_node_with_sufficient_stake(sender) {
            Ok(account) => account,
            Err(e) => return e,
        };

        // Get the current block number.
        let epoch = self.get_epoch();
        let current_block = self.get_block_number() + 1;
        tracing::debug!(
            "received committee selection beacon commit phase timeout transaction at block {} (epoch: {})",
            current_block,
            epoch
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
        // Note that we exclude non-revealing nodes in the previous round from the participating
        // nodes set for this calculation.
        let current_committee = self.committee_info.get(&epoch).unwrap_or_default();
        let participating_nodes = current_committee
            .members
            .iter()
            .filter(|node_index| {
                self.committee_selection_beacon_non_revealing_node
                    .get(*node_index)
                    .is_none()
            })
            .collect::<Vec<_>>();
        let beacons = self.committee_selection_beacon.as_map();
        let committee_beacons = beacons
            .iter()
            .filter(|(node_index, _)| participating_nodes.contains(node_index))
            .collect::<HashMap<_, _>>();

        // Check for sufficient participation; that > 2/3 of the existing committee has committed.
        // Any other non-committee nodes can also optionally participate, but it's not
        // required.
        if committee_beacons.len() > (2 * participating_nodes.len() / 3) {
            // If we have sufficient participation, start the reveal phase.
            let reveal_start = current_block + 1;
            let reveal_end = reveal_start + self.get_reveal_phase_duration();
            tracing::info!(
                "transitioning to committee selection beacon reveal phase because commit phase timed out with sufficient participation (epoch: {}, start: {}, end: {})",
                epoch,
                reveal_start,
                reveal_end
            );
            self.set_committee_selection_beacon_reveal_phase(reveal_start, reveal_end);
        } else {
            // If we don't have sufficient participation, start the commit phase again.

            // Get the current round.
            let round = self.get_committee_selection_beacon_round();
            let Some(round) = round else {
                return TransactionResponse::Revert(
                    ExecutionError::CommitteeSelectionBeaconRoundNotFound,
                );
            };

            // Start the commit phase again by setting the phase metadata.
            let commit_start = current_block + 1;
            let commit_end = commit_start + self.get_commit_phase_duration();
            tracing::info!(
                "transitioning to new committee selection beacon commit phase because commit phase timed out without sufficient participation (epoch: {}, round: {}, start: {}, end: {})",
                epoch,
                round + 1,
                commit_start,
                commit_end,
            );
            tracing::debug!(
                "commit phase had insufficient participation (committed: {:?}, nodes: {:?})",
                committee_beacons.keys().collect::<Vec<_>>(),
                participating_nodes,
            );
            self.set_committee_selection_beacon_commit_phase(commit_start, commit_end);

            // Increment the committee selection beacon round.
            self.set_committee_selection_beacon_round(round + 1);

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
        let _node_index = match self.only_node_with_sufficient_stake(sender) {
            Ok(account) => account,
            Err(e) => return e,
        };

        // Get the current block number.
        let current_block = self.get_block_number() + 1;
        tracing::debug!(
            "received committee selection beacon reveal phase timeout transaction at block {} (epoch: {})",
            current_block,
            self.get_epoch()
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

        // Get the current round.
        let round = self.get_committee_selection_beacon_round();
        let Some(round) = round else {
            return TransactionResponse::Revert(
                ExecutionError::CommitteeSelectionBeaconRoundNotFound,
            );
        };

        // Start the commit phase again.
        let commit_start = current_block + 1;
        let commit_end = commit_start + self.get_commit_phase_duration();
        let epoch = self.get_epoch();
        tracing::info!(
            "transitioning to new committee selection beacon commit phase because reveal phase timed out (epoch: {}, round: {}, start: {}, end: {})",
            epoch,
            round + 1,
            commit_start,
            commit_end
        );
        self.set_committee_selection_beacon_commit_phase(commit_start, commit_end);

        // Increment the committee selection beacon round.
        self.set_committee_selection_beacon_round(round + 1);

        // Save non-revealing nodes to state so we can reject any commits in the next round.
        // Clear existing non-revealing nodes table first.
        self.committee_selection_beacon_non_revealing_node.clear();
        let beacons = self.committee_selection_beacon.as_map();
        let non_revealing_nodes = beacons
            .iter()
            .filter(|(_, (_, reveal))| reveal.is_none())
            .map(|(node_index, _)| *node_index)
            .collect::<Vec<_>>();
        for node_index in &non_revealing_nodes {
            self.committee_selection_beacon_non_revealing_node
                .set(*node_index, ());
        }

        // Slash non-revealing nodes and remove from the committee and active set of nodes if they
        // no longer have sufficient stake.
        let slash_amount = self.get_committee_selection_beacon_non_reveal_slash_amount();
        for node_index in &non_revealing_nodes {
            self.slash_node_after_non_reveal_and_maybe_kick(node_index, &slash_amount);
        }

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

    fn set_committee_selection_beacon_commit_phase(&self, start: u64, end: u64) {
        self.metadata.set(
            Metadata::CommitteeSelectionBeaconPhase,
            Value::CommitteeSelectionBeaconPhase(CommitteeSelectionBeaconPhase::Commit((
                start, end,
            ))),
        );
    }

    fn set_committee_selection_beacon_reveal_phase(&self, start: u64, end: u64) {
        self.metadata.set(
            Metadata::CommitteeSelectionBeaconPhase,
            Value::CommitteeSelectionBeaconPhase(CommitteeSelectionBeaconPhase::Reveal((
                start, end,
            ))),
        );
    }

    fn clear_committee_selection_beacon_phase(&self) {
        self.metadata
            .remove(&Metadata::CommitteeSelectionBeaconPhase);
    }

    fn clear_committee_selection_beacons(&self) {
        self.committee_selection_beacon.clear();
    }

    fn get_committee_selection_beacon_round(&self) -> Option<u64> {
        match self.metadata.get(&Metadata::CommitteeSelectionBeaconRound) {
            Some(Value::CommitteeSelectionBeaconRound(round)) => Some(round),
            None => None,
            _ => unreachable!("invalid committee selection beacon round in metadata"),
        }
    }

    fn set_committee_selection_beacon_round(&self, round: u64) {
        self.metadata.set(
            Metadata::CommitteeSelectionBeaconRound,
            Value::CommitteeSelectionBeaconRound(round),
        );
    }

    fn clear_committee_selection_beacon_round(&self) {
        self.metadata
            .remove(&Metadata::CommitteeSelectionBeaconRound);
    }

    fn execute_epoch_change(
        &self,
        epoch: Epoch,
        current_committee: Committee,
        beacons: FxHashMap<
            NodeIndex,
            (
                CommitteeSelectionBeaconCommit,
                Option<CommitteeSelectionBeaconReveal>,
            ),
        >,
    ) {
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
        let new_committee = self.choose_new_committee(beacons);

        // increment epoch
        let epoch = epoch + 1;

        // Set the new committee, epoch, and reset sub dag index
        self.committee_info.set(epoch, new_committee);

        // Save new epoch to metadata.
        self.metadata.set(Metadata::Epoch, Value::Epoch(epoch));
    }

    fn choose_new_committee(
        &self,
        beacons: FxHashMap<
            NodeIndex,
            (
                CommitteeSelectionBeaconCommit,
                Option<CommitteeSelectionBeaconReveal>,
            ),
        >,
    ) -> Committee {
        let epoch = self.get_epoch();
        let committee_size = self.get_committee_size();

        // Get the active nodes from the node registry.
        let node_registry: Vec<(NodeIndex, NodeInfo)> = self
            .get_node_registry()
            .into_iter()
            .filter(|index| index.1.participation == Participation::True)
            .collect();
        let mut active_nodes: Vec<NodeIndex> = self
            .settle_auction(node_registry)
            .iter()
            .map(|node| node.0)
            .collect();

        // If total number of nodes are less than committee size, all nodes are part of
        // committee. Otherwise, we need to select the committee from the active nodes.
        let committee = if committee_size >= active_nodes.len() as u64 {
            active_nodes.clone()
        } else {
            // Collect the committee selection beacons from the active nodes.
            let mut beacons = beacons
                .into_iter()
                .filter(|(_, (_, reveal))| reveal.is_some())
                .map(|(node_index, (commit, reveal))| (node_index, (commit, reveal.unwrap())))
                .collect::<Vec<_>>();

            // Sort the committee selection beacons by node index.
            beacons.sort_by_key(|k| k.0);

            // Concatenate the reveals in ascending order of node index.
            let combined_reveals = beacons
                .into_iter()
                .map(|(_, (_, reveal))| reveal)
                .collect::<Vec<_>>()
                .concat();
            let combined_reveals_hash: [u8; 32] = Sha3_256::digest(&combined_reveals).into();

            // Shuffle the active nodes using the combined reveals hash as the seed.
            let mut rng: StdRng = SeedableRng::from_seed(combined_reveals_hash);
            active_nodes.shuffle(&mut rng);

            // Take the first `committee_size` nodes from the shuffled list as the new committee.
            active_nodes
                .iter()
                .take(committee_size.try_into().unwrap())
                .copied()
                .collect()
        };

        // Calculate the epoch end timestamp.
        let epoch_time = self.get_epoch_time();
        let epoch_end_timestamp =
            self.committee_info.get(&epoch).unwrap().epoch_end_timestamp + epoch_time;

        Committee {
            ready_to_change: Vec::with_capacity(committee.len()),
            members: committee,
            epoch_end_timestamp,
            active_node_set: active_nodes,
            node_registry_changes: Default::default(),
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
                    tracing::debug!("node {:?} has uptime lower than minimum uptime", node);
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

    fn get_epoch_time(&self) -> u64 {
        match self.parameters.get(&ProtocolParamKey::EpochTime) {
            Some(ProtocolParamValue::EpochTime(epoch_time)) => epoch_time,
            _ => unreachable!("invalid epoch time in protocol parameters"),
        }
    }

    fn get_committee_size(&self) -> u64 {
        match self.parameters.get(&ProtocolParamKey::CommitteeSize) {
            Some(ProtocolParamValue::CommitteeSize(committee_size)) => committee_size,
            _ => unreachable!("invalid committee size in protocol parameters"),
        }
    }

    /// Slash a node by removing the given amount from the node's staked balance.
    ///
    /// If the node no longer has sufficient stake, it's removed from the committee and active node
    /// set.
    pub fn slash_node_after_non_reveal_and_maybe_kick(
        &self,
        node_index: &NodeIndex,
        amount: &HpUfixed<18>,
    ) {
        let node_index = *node_index;

        // Remove the given slash amount from the node's staked balance.
        let mut node_info = self.node_info.get(&node_index).unwrap();
        let remaining_amount = if node_info.stake.staked >= amount.clone() {
            node_info.stake.staked -= amount.clone();
            HpUfixed::<18>::zero()
        } else {
            let remaining = amount.clone() - node_info.stake.staked;
            node_info.stake.staked = HpUfixed::zero();
            remaining
        };
        if node_info.stake.locked >= remaining_amount {
            node_info.stake.locked -= remaining_amount;
        } else {
            node_info.stake.locked = HpUfixed::zero();
        }
        self.node_info.set(node_index, node_info.clone());
        tracing::info!("slashing node {:?} by {:?}", node_index, amount);

        // Record the node registry change.
        self.record_node_registry_change(
            node_info.public_key,
            NodeRegistryChange::Slashed((
                amount.clone(),
                node_info.stake,
                NodeRegistryChangeSlashReason::CommitteeBeaconNonReveal,
            )),
        );

        // If the node no longer has sufficient stake, remove it from the committee members and
        // active nodes.
        if !self.has_sufficient_stake(&node_index) {
            let epoch = self.get_epoch();
            let mut committee = self.get_committee(epoch);

            // Remove node from committee members and active node set.
            committee.members.retain(|member| *member != node_index);
            committee
                .active_node_set
                .retain(|member| *member != node_index);

            // Save the updated committee info.
            tracing::info!(
                "removing node {:?} from committee and active node set after being slashed",
                node_index
            );
            self.committee_info.set(epoch, committee);
        }
    }

    /// Get the slash amount for non-revealing nodes in the committee selection beacon process.
    ///
    /// Returns 0 if the slash amount is not set in the protocol parameters.
    /// Panics if the slash amount is not the expected type.
    fn get_committee_selection_beacon_non_reveal_slash_amount(&self) -> HpUfixed<18> {
        match self
            .parameters
            .get(&ProtocolParamKey::CommitteeSelectionBeaconNonRevealSlashAmount)
        {
            Some(ProtocolParamValue::CommitteeSelectionBeaconNonRevealSlashAmount(amount)) => {
                amount.into()
            },
            None => HpUfixed::<18>::zero(),
            _ => unreachable!(
                "invalid committee selection beacon non-reveal slash amount in protocol parameters"
            ),
        }
    }
}
