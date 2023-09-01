use fleek_crypto::{ClientPublicKey, ClientSignature};
use lightning_handshake::handshake::Handshake;
use lightning_handshake::schema;
use lightning_handshake::transports::mock::dial_mock;
use lightning_interfaces::infu_collection::{Collection, Node};
use lightning_interfaces::partial;
use lightning_service_executor::shim::ServiceExecutor;
use lightning_test_utils::json_config::JsonConfigProvider;
use lightning_test_utils::keys::KeyOnlySigner;
use serde_json::json;

partial!(TestBinding {
    SignerInterface = KeyOnlySigner;
    ConfigProviderInterface = JsonConfigProvider;
    HandshakeInterface = Handshake<Self>;
    ServiceExecutorInterface = ServiceExecutor<Self>;
});

#[tokio::test]
async fn demo() -> anyhow::Result<()> {
    let config: JsonConfigProvider = json!({
      "handshake": {
        "transport": [
          {
            "type": "Mock",
            "port": 69
          },
        ],
        "worker": [
          {
            "type": "AsyncWorker"
          },
        ]
      },
      "service-executor": {
        "services": [0]
      },
    })
    .into();

    let node = Node::<TestBinding>::init(config).unwrap();

    node.start().await;

    let client = dial_mock(69).await.unwrap();

    // send the initial handshake
    client
        .0
        .send(
            schema::HandshakeRequestFrame::Handshake {
                retry: None,
                service: 0,
                pk: ClientPublicKey([1; 96]),
                pop: ClientSignature([2; 48]),
            }
            .encode(),
        )
        .await?;

    node.shutdown().await;

    Ok(())
}
