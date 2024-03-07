use fleek_crypto::NodePublicKey;
use futures::future::BoxFuture;

use super::MuxerInterface;

enum Message {
    Handshake { pk: NodePublicKey },
    NewStream { bi: bool, sid: i16 },
    AcceptStream { sid: i16 },
    CloseStream { sid: i16 },
    Payload { sid: i16, data: Vec<u8> },
}

struct SimulonMuxer {
    pk: NodePublicKey,
    listener: tokio::sync::mpsc::Recivie<MuxConnection>,
}

struct MuxConnection {
    connection: simulon::api::Connection,
    pk: NodePublicKey,
    did_we_establish_conn: bool, // =true  | =false
                                 // sid=1  | sid=-1
    next_sid: i16 // 0
}

impl MuxConnecction {
    pub fn get_next_sid(&mut self) -> i16 {
        if self.did_we_estbalish_conn {
            self.next_sid += 1;
        } else {
            self.next_sid -= 1;
        }
        self.next_sid
    }
}

impl MuxerInterface for SimulonMuxer {
    type Connecting = BoxFuture<io::Result<Self::Connection>>;
    type Connection: MuxConnection;
    type Config;

    fn init(config: Self::Config) -> io::Result<Self> {
        let (tx, rx) = chan();
        simulon::api::spawn(async move {
            let mut listener = simulon::listen(0);
            while let Some(conn) = listener.accept().await {
                simulon::api::spawn(async move {
                    if let Ok(Message::Handshake { pk }) = conn.recv::<Message>() {
                        tx.send(MuxConnection {
                            connection: conn
                            pk,
                            did_we_establish_conn: false
                        });
                    } else {
                        unreachble!();
                    }
                });
            }
        });

        Ok(Self {
            listener:
        })
    }

    fn connect(&self, peer: NodeInfo, server_name: &str) -> Self::Connecting {
        let pk = self.pk.clone();
        Box::pin(async move {
            let connection = simulon::connect(..).await?;
            connection.write(Message::Handshake {
                pk
            }).await;
            Ok(connectiob)
        })
    }

    fn accept(&self) -> Self::Connecting {
        let rx = self.listener.clone();
        Box::pin(async move {
            rx.recv()
        })
    }

    fn close(&self) -> impl ::core::future::Future<Output = ()> + Send {
        todo!()
    }
}
