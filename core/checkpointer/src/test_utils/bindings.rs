use lightning_application::Application;
use lightning_broadcast::Broadcast;
use lightning_forwarder::Forwarder;
use lightning_interfaces::partial;
use lightning_notifier::Notifier;
use lightning_pool::PoolProvider;
use lightning_rep_collector::ReputationAggregator;
use lightning_signer::Signer;
use lightning_test_utils::json_config::JsonConfigProvider;
use lightning_test_utils::keys::EphemeralKeystore;
use lightning_topology::Topology;

use crate::Checkpointer;

partial!(TestNodeComponents {
    ApplicationInterface = Application<Self>;
    BroadcastInterface = Broadcast<Self>;
    CheckpointerInterface = Checkpointer<Self>;
    ConfigProviderInterface = JsonConfigProvider;
    ForwarderInterface = Forwarder<Self>;
    KeystoreInterface = EphemeralKeystore<Self>;
    NotifierInterface = Notifier<Self>;
    PoolInterface = PoolProvider<Self>;
    ReputationAggregatorInterface = ReputationAggregator<Self>;
    TopologyInterface = Topology<Self>;
    SignerInterface = Signer<Self>;
});
