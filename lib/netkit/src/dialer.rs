use std::collections::HashMap;
use std::future::Future;

use fleek_crypto::NodePublicKey;
use futures::stream::FuturesUnordered;
use quinn::{Connection, Endpoint};

use crate::endpoint::NodeAddress;

/// Connects to peers.
pub struct Dialer {}
