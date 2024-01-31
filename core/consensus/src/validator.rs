// Copyright 2022-2023 Fleek Network
// SPDX-License-Identifier: Apache-2.0, MIT

use anyhow::{anyhow, Context, Error, Result};
use async_trait::async_trait;
use fleek_crypto::{ConsensusPublicKey, EthAddress, NodePublicKey, TransactionSender};
use lightning_interfaces::types::{
    TransactionRequest,
    UpdateMethod,
    UpdateRequest,
    MAX_DELIVERY_ACKNOWLEDGMENTS,
    MAX_MEASUREMENTS_PER_TX,
    MAX_UPDATES_CONTENT_REGISTRY,
};
use lightning_interfaces::ToDigest;
use lightning_utils::eth;
use narwhal_types::Batch;
use narwhal_worker::TransactionValidator;
use sui_protocol_config::ProtocolConfig;

#[derive(Clone)]
pub struct Validator {
    node_public_key: NodePublicKey,
    consensus_public_key: ConsensusPublicKey,
}

impl Validator {
    pub fn new(node_public_key: NodePublicKey, consensus_public_key: ConsensusPublicKey) -> Self {
        Self {
            node_public_key,
            consensus_public_key,
        }
    }
}

#[async_trait]
impl TransactionValidator for Validator {
    type Error = Error;

    fn validate(&self, t: &[u8]) -> Result<()> {
        self.validate_txn(t, true)
    }

    async fn validate_batch(&self, b: &Batch, _protocol_config: &ProtocolConfig) -> Result<()> {
        let txns = match b {
            Batch::V1(batch) => &batch.transactions,
            Batch::V2(batch) => &batch.transactions,
        };
        for txn in txns {
            self.validate_txn(txn, false)?;
        }
        Ok(())
    }
}

impl Validator {
    fn validate_txn(&self, t: &[u8], mempool: bool) -> Result<()> {
        match TransactionRequest::try_from(t).context("Failed to deserialize transaction")? {
            TransactionRequest::UpdateRequest(UpdateRequest { signature, payload }) => {
                // TODO(matthias): before checking anything, we should enforce a hard cap on
                // transaction size.

                let digest = payload.to_digest();
                if !payload.sender.verify(signature, &digest) {
                    return Err(anyhow!("Invalid signature"));
                }

                if mempool {
                    // Skip further checks if the transaction was sent from this node.
                    // We only do this if the transaction was sent to the node's local mempool.
                    match payload.sender {
                        TransactionSender::NodeMain(public_key) => {
                            if public_key == self.node_public_key {
                                return Ok(());
                            }
                        },
                        TransactionSender::NodeConsensus(public_key) => {
                            if public_key == self.consensus_public_key {
                                return Ok(());
                            }
                        },
                        _ => (),
                    }
                }

                match payload.method {
                    UpdateMethod::SubmitReputationMeasurements { measurements } => {
                        if measurements.len() > MAX_MEASUREMENTS_PER_TX {
                            return Err(anyhow!("Too many reputation measurements"));
                        }
                    },
                    UpdateMethod::UpdateContentRegistry { updates } => {
                        if updates.len() > MAX_UPDATES_CONTENT_REGISTRY {
                            return Err(anyhow!("Too many updates"));
                        }
                    },
                    UpdateMethod::SubmitDeliveryAcknowledgmentAggregation {
                        commodity: _,
                        service_id: _,
                        proofs,
                        metadata: _,
                        hashes,
                    } => {
                        if proofs.len() > MAX_DELIVERY_ACKNOWLEDGMENTS
                            || hashes.len() > MAX_DELIVERY_ACKNOWLEDGMENTS
                        {
                            return Err(anyhow!("Too many delivery acknowledgments"));
                        }
                    },
                    _ => (),
                }
            },
            TransactionRequest::EthereumRequest(mut eth_tx) => {
                let sender: EthAddress = if let Ok(address) = eth_tx.tx.recover_from_mut() {
                    address.0.into()
                } else {
                    return Err(anyhow!("Invalid signature"));
                };
                if !eth::verify_signature(&eth_tx.tx, sender) {
                    return Err(anyhow!("Invalid signature"));
                }
            },
        }
        Ok(())
    }
}
