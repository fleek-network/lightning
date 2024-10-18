use anyhow::Result;
use lightning_interfaces::{NodeComponents, RpcInterface};
use lightning_rpc::{load_hmac_secret, RpcClient};

use super::TestFullNode;

impl<C: NodeComponents> TestFullNode<C> {
    pub fn rpc_client(&self) -> Result<RpcClient> {
        let addr = self.rpc().listen_address().expect("rpc not ready");
        RpcClient::new_no_auth(&format!("http://{}", addr))
    }

    pub async fn rpc_admin_client(&self) -> Result<RpcClient> {
        let secret = load_hmac_secret(Some(self.home_dir.clone()))?;
        let addr = self.rpc().listen_address().expect("rpc not ready");
        RpcClient::new(&format!("http://{}/admin", addr), Some(&secret)).await
    }

    pub async fn rpc_ws_client(&self) -> Result<jsonrpsee::ws_client::WsClient> {
        let addr = self.rpc().listen_address().expect("rpc not ready");
        jsonrpsee::ws_client::WsClientBuilder::default()
            .build(&format!("ws://{}", addr))
            .await
            .map_err(|e| anyhow::anyhow!(e))
    }
}
