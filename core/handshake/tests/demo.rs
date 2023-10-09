use fleek_crypto::{ClientPublicKey, ClientSignature};
use lightning_blockstore::blockstore::Blockstore;
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
    BlockStoreInterface = Blockstore<Self>;
    ConfigProviderInterface = JsonConfigProvider;
    HandshakeInterface = Handshake<Self>;
    ServiceExecutorInterface = ServiceExecutor<Self>;
});

#[tokio::test]
async fn demo() -> anyhow::Result<()> {
    let config: JsonConfigProvider = json!({
      "handshake": {
        "http_address": "127.0.0.1:4220",
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
        ],
        "http_addr": "0.0.0.0:4210"
      },
      "service-executor": {
        "services": [0]
      },

    })
    .into();

    let node = Node::<TestBinding>::init(config).unwrap();

    node.start().await;

    {
        let (tx, mut rx) = dial_mock(69).await.unwrap();

        // send the initial handshake
        tx.send(
            schema::HandshakeRequestFrame::Handshake {
                retry: None,
                service: 0,
                pk: ClientPublicKey([1; 96]),
                pop: ClientSignature([2; 48]),
            }
            .encode(),
        )
        .await?;

        // let response = rx.recv().await;
        // println!("handshake response {response:?}");

        // send a message.
        tx.send(
            schema::RequestFrame::ServicePayload {
                bytes: "Hello Service".to_owned().into(),
            }
            .encode(),
        )
        .await?;

        let response = rx.recv().await.unwrap();
        let response = schema::ResponseFrame::decode(&response).unwrap();
        println!("request response {response:?}");
    }

    node.shutdown().await;

    Ok(())
}
