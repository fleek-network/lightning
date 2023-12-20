// Copyright 2022-2023 Fleek Network
// SPDX-License-Identifier: Apache-2.0, MIT

use anyhow::{anyhow, Context, Error, Result};
use async_trait::async_trait;
use fleek_crypto::{ConsensusPublicKey, EthAddress, NodePublicKey, TransactionSender};
use lightning_interfaces::types::{TransactionRequest, UpdateRequest};
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
        match TransactionRequest::try_from(t).context("Failed to deserialize transaction")? {
            TransactionRequest::UpdateRequest(UpdateRequest { signature, payload }) => {
                let digest = payload.to_digest();
                if !payload.sender.verify(signature, &digest) {
                    return Err(anyhow!("Invalid signature"));
                }

                // skip further checks if the transaction was sent from this node
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

    async fn validate_batch(&self, _b: &Batch, _protocol_config: &ProtocolConfig) -> Result<()> {
        Ok(())
    }
}
