use jsonrpsee::core::RpcResult;
use jsonrpsee::proc_macros::rpc;
use lightning_firewall::FirewallCommand;
use lightning_interfaces::types::Blake3Hash;

#[rpc(client, server, namespace = "admin")]
pub trait AdminApi {
    #[method(name = "store")]
    async fn store(&self, path: String) -> RpcResult<Blake3Hash>;

    /// Queue a firewall command to be executed, doesnt wait for a resposne
    #[method(name = "queue_firewall_command")]
    async fn queue_firewall_command(&self, name: &str, command: FirewallCommand) -> RpcResult<()>;

    /// Queue a firewall command to be executed, waits for a response
    #[method(name = "queue_firewall_command_and_wait")]
    async fn queue_firewall_command_and_wait_for_success(
        &self,
        name: &str,
        command: FirewallCommand,
    ) -> RpcResult<()>;

    #[method(name = "ping")]
    async fn ping(&self) -> RpcResult<String>;
}
