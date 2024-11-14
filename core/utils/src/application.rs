use std::collections::{HashMap, HashSet};
use std::time::Duration;

use fleek_crypto::{EthAddress, NodePublicKey};
use hp_fixed::unsigned::HpUfixed;
use lightning_interfaces::prelude::*;
use lightning_interfaces::types::{
    Epoch,
    EpochInfo,
    Metadata,
    NodeIndex,
    NodeInfo,
    NodeInfoWithIndex,
    ProtocolParamKey,
    ProtocolParamValue,
    Value,
};
use lightning_interfaces::PagingParams;
use types::{CommitteeSelectionBeaconPhase, EpochEra, Participation};

pub trait QueryRunnerExt: SyncQueryRunnerInterface {
    /// Returns the chain id
    fn get_chain_id(&self) -> u32 {
        match self.get_metadata(&Metadata::ChainId) {
            Some(Value::ChainId(id)) => id,
            _ => 0,
        }
    }

    /// Returns the committee members of the current epoch
    fn get_committee_members(&self) -> Vec<NodePublicKey> {
        self.get_committee_members_by_index()
            .into_iter()
            .filter_map(|node_index| self.index_to_pubkey(&node_index))
            .collect()
    }

    /// Returns the committee members of the current epoch by NodeIndex
    fn get_committee_members_by_index(&self) -> Vec<NodeIndex> {
        let epoch = self.get_current_epoch();
        self.get_committee_info(&epoch, |c| c.members)
            .unwrap_or_default()
    }

    /// Get Current Epoch
    /// Returns just the current epoch
    fn get_current_epoch(&self) -> Epoch {
        match self.get_metadata(&Metadata::Epoch) {
            Some(Value::Epoch(epoch)) => epoch,
            _ => 0,
        }
    }

    /// Get current epoch era.
    fn get_epoch_era(&self) -> EpochEra {
        match self.get_metadata(&Metadata::EpochEra) {
            Some(Value::EpochEra(era)) => era,
            _ => 0,
        }
    }

    /// Get Current Epoch Info
    /// Returns all the information on the current epoch that Narwhal needs to run
    fn get_epoch_info(&self) -> EpochInfo {
        let epoch = self.get_current_epoch();
        let epoch_era = self.get_epoch_era();
        // look up current committee
        let committee = self.get_committee_info(&epoch, |c| c).unwrap_or_default();
        EpochInfo {
            committee: committee
                .members
                .iter()
                .filter_map(|member| self.get_node_info::<NodeInfo>(member, |n| n))
                .collect(),
            epoch,
            epoch_era,
            epoch_end: committee.epoch_end_timestamp,
        }
    }

    /// Return all latencies measurements for the current epoch.
    fn get_current_latencies(&self) -> HashMap<(NodePublicKey, NodePublicKey), Duration> {
        self.get_latencies_iter::<HashMap<(NodePublicKey, NodePublicKey), Duration>>(
            |latencies| -> HashMap<(NodePublicKey, NodePublicKey), Duration> {
                latencies
                    .filter_map(|nodes| self.get_latencies(&nodes).map(|latency| (nodes, latency)))
                    .filter_map(|((index_lhs, index_rhs), latency)| {
                        let node_lhs =
                            self.get_node_info::<NodePublicKey>(&index_lhs, |n| n.public_key);
                        let node_rhs =
                            self.get_node_info::<NodePublicKey>(&index_rhs, |n| n.public_key);
                        match (node_lhs, node_rhs) {
                            (Some(node_lhs), Some(node_rhs)) => {
                                Some(((node_lhs, node_rhs), latency))
                            },
                            _ => None,
                        }
                    })
                    .collect()
            },
        )
    }

    /// Returns the node info of the genesis committee members
    fn get_genesis_committee(&self) -> Vec<(NodeIndex, NodeInfo)> {
        match self.get_metadata(&Metadata::GenesisCommittee) {
            Some(Value::GenesisCommittee(committee)) => committee
                .iter()
                .filter_map(|node_index| {
                    self.get_node_info::<NodeInfo>(node_index, |n| n)
                        .map(|node_info| (*node_index, node_info))
                })
                .collect(),
            _ => {
                // unreachable seeded at genesis
                Vec::new()
            },
        }
    }

    /// Returns the latest block number.
    fn get_block_number(&self) -> Option<u64> {
        match self.get_metadata(&Metadata::BlockNumber) {
            Some(Value::BlockNumber(block)) => Some(block),
            None => None,
            _ => unreachable!("invalid block number in metadata"),
        }
    }

    /// Returns last executed block hash. [0;32] is genesis
    fn get_last_block(&self) -> [u8; 32] {
        match self.get_metadata(&Metadata::LastBlockHash) {
            Some(Value::Hash(hash)) => hash,
            _ => [0; 32],
        }
    }

    /// Returns the current sub dag index
    fn get_sub_dag_index(&self) -> u64 {
        if let Some(Value::SubDagIndex(value)) = self.get_metadata(&Metadata::SubDagIndex) {
            value
        } else {
            0
        }
    }

    /// Returns the current sub dag round
    fn get_sub_dag_round(&self) -> u64 {
        if let Some(Value::SubDagRound(value)) = self.get_metadata(&Metadata::SubDagRound) {
            value
        } else {
            0
        }
    }

    /// Returns a full copy of the entire node-registry,
    /// Paging Params - filtering nodes that are still a valid node and have enough stake; Takes
    /// from starting index and specified amount.
    fn get_node_registry(&self, paging: Option<PagingParams>) -> Vec<NodeInfoWithIndex> {
        let staking_amount = self.get_staking_amount().into();

        self.get_node_table_iter::<Vec<NodeInfoWithIndex>>(|nodes| -> Vec<NodeInfoWithIndex> {
            let nodes = nodes.map(|index| NodeInfoWithIndex {
                index,
                info: self.get_node_info::<NodeInfo>(&index, |n| n).unwrap(),
            });
            match paging {
                None => nodes
                    .filter(|node| node.info.stake.staked >= staking_amount)
                    .collect(),
                Some(PagingParams {
                    ignore_stake,
                    limit,
                    start,
                }) => {
                    let mut nodes = nodes
                        .filter(|node| ignore_stake || node.info.stake.staked >= staking_amount)
                        .collect::<Vec<NodeInfoWithIndex>>();

                    nodes.sort_by_key(|info| info.index);

                    nodes
                        .into_iter()
                        .filter(|info| info.index >= start)
                        .take(limit)
                        .collect()
                },
            }
        })
    }

    /// Gets the current active node set for a given epoch
    fn get_active_nodes(&self) -> Vec<NodeInfoWithIndex> {
        self.get_active_node_set()
            .iter()
            .filter_map(|index| {
                self.get_node_info(index, |node_info| node_info)
                    .map(|info| NodeInfoWithIndex {
                        index: *index,
                        info,
                    })
            })
            .collect()
    }

    /// Get the active node indexes.
    fn get_active_node_set(&self) -> HashSet<NodeIndex> {
        let current_epoch = self.get_current_epoch();
        self.get_committee_info(&current_epoch, |committee| committee.active_node_set)
            .unwrap_or_default()
            .into_iter()
            .collect()
    }

    /// Returns the amount that is required to be a valid node in the network.
    fn get_staking_amount(&self) -> u64 {
        match self.get_protocol_param(&ProtocolParamKey::MinimumNodeStake) {
            Some(ProtocolParamValue::MinimumNodeStake(min_stake)) => min_stake,
            _ => 0,
        }
    }

    /// Returns true if the node is a valid node in the network, with enough stake.
    fn has_sufficient_stake(&self, id: &NodePublicKey) -> bool {
        let minimum_stake_amount = self.get_staking_amount().into();
        self.pubkey_to_index(id).is_some_and(|node_idx| {
            self.get_node_info(&node_idx, |n| n.stake)
                .is_some_and(|stake| stake.staked + stake.locked >= minimum_stake_amount)
        })
    }

    /// Returns true if the node is participating.
    fn is_participating_node(&self, id: &NodePublicKey) -> bool {
        self.pubkey_to_index(id).is_some_and(|node_idx| {
            self.get_node_info(&node_idx, |n| n.participation)
                .is_some_and(|participation| {
                    matches!(participation, Participation::OptedIn | Participation::True)
                })
        })
    }

    /// Returns the protocol fund address.
    fn get_protocol_fund_address(&self) -> Option<EthAddress> {
        match self.get_metadata(&Metadata::ProtocolFundAddress) {
            Some(Value::AccountPublicKey(value)) => Some(value),
            None => None,
            _ => unreachable!("invalid protocol fund address in metadata"),
        }
    }

    /// Returns the total supply of FLK.
    fn get_total_supply(&self) -> Option<HpUfixed<18>> {
        match self.get_metadata(&Metadata::TotalSupply) {
            Some(Value::HpUfixed(value)) => Some(value),
            None => None,
            _ => unreachable!("invalid total supply in metadata"),
        }
    }

    /// Returns the supply at the start of the year.
    fn get_supply_year_start(&self) -> Option<HpUfixed<18>> {
        match self.get_metadata(&Metadata::SupplyYearStart) {
            Some(Value::HpUfixed(value)) => Some(value),
            None => None,
            _ => unreachable!("invalid supply year start in metadata"),
        }
    }

    /// Returns the ping timeout from the protocol parameters.
    fn get_reputation_ping_timeout(&self) -> Duration {
        match self.get_protocol_param(&ProtocolParamKey::ReputationPingTimeout) {
            Some(ProtocolParamValue::ReputationPingTimeout(timeout)) => timeout,
            // Default to 15s for backwards compatibility.
            None => Duration::from_secs(15),
            _ => unreachable!(),
        }
    }

    /// Returns the target k for the topology.
    fn get_topology_target_k(&self) -> usize {
        match self.get_protocol_param(&ProtocolParamKey::TopologyTargetK) {
            Some(ProtocolParamValue::TopologyTargetK(target_k)) => target_k,
            // Default to 8 for backwards compatibility.
            _ => 8,
        }
    }

    /// Returns the minimum number of nodes for the topology.
    fn get_topology_min_nodes(&self) -> usize {
        match self.get_protocol_param(&ProtocolParamKey::TopologyMinNodes) {
            Some(ProtocolParamValue::TopologyMinNodes(min_nodes)) => min_nodes,
            // Default to 16 for backwards compatibility.
            _ => 16,
        }
    }

    /// Returns the current phase of the committee selection beacon.
    fn get_committee_selection_beacon_phase(&self) -> Option<CommitteeSelectionBeaconPhase> {
        match self.get_metadata(&Metadata::CommitteeSelectionBeaconPhase) {
            Some(Value::CommitteeSelectionBeaconPhase(phase)) => Some(phase),
            None => None,
            _ => unreachable!("invalid committee selection beacon phase in metadata"),
        }
    }

    /// Returns the current round of the committee selection beacon.
    fn get_committee_selection_beacon_round(&self) -> Option<u64> {
        match self.get_metadata(&Metadata::CommitteeSelectionBeaconRound) {
            Some(Value::CommitteeSelectionBeaconRound(round)) => Some(round),
            None => None,
            _ => unreachable!("invalid committee selection beacon round in metadata"),
        }
    }
}

impl<T: SyncQueryRunnerInterface> QueryRunnerExt for T {}
