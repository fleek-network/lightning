use lightning_application::Application;
use lightning_broadcast::Broadcast;
use lightning_forwarder::Forwarder;
use lightning_interfaces::partial;
use lightning_notifier::Notifier;
use lightning_pool::PoolProvider;
use lightning_rep_collector::ReputationAggregator;
use lightning_rpc::Rpc;
use lightning_signer::Signer;
use lightning_topology::Topology;

use crate::json_config::JsonConfigProvider;
use crate::keys::EphemeralKeystore;

partial!(TestNodeComponents {
    ApplicationInterface = Application<Self>;
    BroadcastInterface = Broadcast<Self>;
    ConfigProviderInterface = JsonConfigProvider;
    ForwarderInterface = Forwarder<Self>;
    KeystoreInterface = EphemeralKeystore<Self>;
    NotifierInterface = Notifier<Self>;
    PoolInterface = PoolProvider<Self>;
    ReputationAggregatorInterface = ReputationAggregator<Self>;
    RpcInterface = Rpc<Self>;
    SignerInterface = Signer<Self>;
    TopologyInterface = Topology<Self>;
});
