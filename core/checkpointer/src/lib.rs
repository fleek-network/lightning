//! The [`Checkpointer`] is a component that produces checkpoint attestations and aggregates them
//! when a supermajority is reached.
//!
//! The goal of the checkpointer is to provide a mechanism for clients to efficiently and securely
//! verify the blockchain state without traversing the history of transactions. Checkpoints at every
//! epoch contain a state root, with an aggregate BLS signature over it, that a supermajority of
//! nodes have attested to. Clients can use these state roots to verify inclusion or non-inclusion
//! of anything in the state at each epoch.
//!
//! The checkpointer listens for epoch change notifications from the notifier and checkpoint
//! attestation messages from the broadcaster. It's responsible for creating checkpoint attestations
//! and checking for a supermajority of attestations for the epoch, aggregating the signatures if a
//! supermajority is reached, and saving the aggregate checkpoint to the database as the canonical
//! checkpoint for the epoch. The aggregate checkpoint contains a state root that can be used by
//! clients to verify the blockchain state using merkle proofs.

mod attestation_listener;
mod checkpointer;
mod config;
mod database;
mod epoch_change_listener;
mod message;
mod query;
mod rocks;

#[cfg(test)]
mod test_utils;
#[cfg(test)]
mod tests;

pub use checkpointer::*;
pub use config::*;
pub use query::*;
