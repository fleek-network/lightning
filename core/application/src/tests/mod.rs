use lightning_interfaces::partial_node_components;
use lightning_test_utils::json_config::JsonConfigProvider;
use lightning_test_utils::keys::EphemeralKeystore;

use crate::Application;

mod balances;
mod committee_beacon;
mod content_registry;
mod epoch_change;
mod everything_else;
mod genesis;
mod participation;
mod pod;
mod protocol_params;
mod reputation;
mod staking;
mod utils;

partial_node_components!(TestBinding {
    ConfigProviderInterface = JsonConfigProvider;
    ApplicationInterface = Application<Self>;
    KeystoreInterface = EphemeralKeystore<Self>;
});
