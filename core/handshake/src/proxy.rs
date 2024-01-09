use anyhow::{anyhow, Result};
use arrayref::array_ref;
use async_channel::Receiver;
use bytes::BytesMut;
use lightning_interfaces::ExecutorProviderInterface;
use lightning_schema::handshake::{ResponseFrame, TerminationReason};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;
use tokio::task::JoinHandle;
use tracing::error;

use crate::handshake::Context;
use crate::schema::RequestFrame;
use crate::transports::{match_transport, TransportPair, TransportReceiver, TransportSender};

/// A proxy for a session with a single primary connection
// TODO: Every single error state should have a termination reason
pub struct Proxy<S: TransportSender, R: TransportReceiver, P: ExecutorProviderInterface> {
    sender: S,
    receiver: R,
    socket: UnixStream,
    socket_buffer: BytesMut,
    current_write: usize,
    secondary_rx: Receiver<TransportPair>,
    token: [u8; 48],
    context: Context<P>,
}

/// Shared handler for forwarding outgoing payloads from the service socket to a transport
#[inline(always)]
fn handle_socket_bytes<S: TransportSender>(
    socket_buffer: &mut BytesMut,
    current_write: &mut usize,
    sender: &mut S,
) -> Result<()> {
    while !socket_buffer.is_empty() {
        if *current_write > 0 {
            // write bytes to the transport
            let len = socket_buffer.len().min(*current_write);
            let bytes = socket_buffer.split_to(len);
            *current_write -= len;
            sender.write(&bytes)?;
        } else if socket_buffer.len() >= 4 {
            // read the payload delimiter
            let bytes = socket_buffer.split_to(4);
            let len = u32::from_be_bytes(*array_ref![bytes, 0, 4]) as usize;
            sender.start_write(len);
            *current_write = len;
            socket_buffer.reserve(len);
        } else {
            // not enough bytes to do anything more
            break;
        }
    }

    Ok(())
}

impl<S: TransportSender, R: TransportReceiver, P: ExecutorProviderInterface> Drop
    for Proxy<S, R, P>
{
    fn drop(&mut self) {
        // cleanup shared state with the transport context
        self.context.cleanup_connection(&self.token);
    }
}

impl<S: TransportSender, R: TransportReceiver, P: ExecutorProviderInterface> Proxy<S, R, P> {
    #[allow(clippy::too_many_arguments)]
    #[inline(always)]
    pub fn new(
        sender: S,
        receiver: R,
        socket: UnixStream,
        secondary_rx: Receiver<TransportPair>,
        token: [u8; 48],
        context: Context<P>,
    ) -> Self {
        Self {
            sender,
            receiver,
            secondary_rx,
            socket,
            current_write: 0,
            token,
            context,
            socket_buffer: BytesMut::new(),
        }
    }

    /// Spawn the proxy task for the connection, and cleanup after it completes
    #[inline(always)]
    pub fn spawn(self) -> JoinHandle<()> {
        tokio::spawn(async move {
            // run event loop
            if let Err(e) = self.run().await {
                error!("connection proxy encountered an error: {e}");
            }
        })
    }

    fn terminate(&mut self, reason: TerminationReason) {
        self.sender.send(ResponseFrame::Termination { reason })
    }

    /// Main loop, handling incoming frames and outgoing bytes until the shutdown
    /// signal is received or an error occurs.
    async fn run(mut self) -> Result<()> {
        loop {
            tokio::select! {
                // Handle incoming payloads
                res = self.receiver.recv() => match res {
                    Some(req) => self.handle_incoming(req).await?,
                    None => break Err(anyhow!("primary connection disconnected")),
                },
                // Handle outgoing socket bytes from the service
                res = self.socket.read_buf(&mut self.socket_buffer) => match res {
                    Ok(0) => break Ok(()),
                    Ok(_) => {
                        handle_socket_bytes(
                            &mut self.socket_buffer,
                            &mut self.current_write,
                            &mut self.sender
                        )?
                    },
                    Err(e) => {
                        self.terminate(TerminationReason::InternalError);
                        break Err(e.into())
                    },
                },
                // Handle a secondary connection joining the session
                res = self.secondary_rx.recv() => match res {
                    Ok(pair) => {
                        break self.into_secondary_proxy(pair).await;
                        // TODO: Continue original proxy loop for the primary connection after the
                        // secondary connection ends. If there is an incomplete payload to the secondary,
                        // flush it.
                    },
                    Err(e) => {
                        self.terminate(TerminationReason::InternalError);
                        break Err(e.into())
                    },
                },
                // Shutdown signal from the node
                _ = self.context.shutdown.wait_for_shutdown() => {
                    self.terminate(TerminationReason::Shutdown);
                    break Ok(())
                },
            }
        }
    }

    /// Handle incoming frames from the transport
    async fn handle_incoming(&mut self, req: RequestFrame) -> Result<()> {
        match req {
            RequestFrame::ServicePayload { bytes } => {
                // write delimiter and payload to the socket
                self.socket
                    .write_u32(bytes.len() as u32)
                    .await
                    .map_err(|e| {
                        self.terminate(TerminationReason::InvalidToken);
                        e
                    })?;
                self.socket.write_all(&bytes).await.map_err(|e| {
                    self.terminate(TerminationReason::InvalidToken);
                    e
                })?;
            },
            RequestFrame::AccessToken { ttl } => {
                let ttl = self
                    .context
                    .handle_access_token_request(&self.token, ttl)
                    .await
                    .map_err(|e| {
                        self.terminate(TerminationReason::InvalidToken);
                        e
                    })?;

                self.sender.send(ResponseFrame::AccessToken {
                    ttl,
                    access_token: self.token.into(),
                });
            },
            RequestFrame::ExtendAccessToken { ttl } => self
                .context
                .handle_extend_access_token_request(&self.token, ttl)
                .await
                .map_err(|e| {
                    self.terminate(TerminationReason::InvalidToken);
                    e
                })?,
            RequestFrame::DeliveryAcknowledgment {} => todo!("verify and submit client DACK"),
            _ => unimplemented!(),
        }

        Ok(())
    }

    /// Transform the single connection proxy into a proxy with secondary connection,
    /// and spawn the new run loop
    async fn into_secondary_proxy(mut self, pair: TransportPair) -> Result<()> {
        if self.current_write != 0 {
            // Read and flush the remaining bytes from the socket to the primary connection
            while self.socket_buffer.len() < self.current_write {
                if matches!(
                    self.socket.read_buf(&mut self.socket_buffer).await,
                    Ok(0) | Err(_)
                ) {
                    self.terminate(TerminationReason::InternalError);
                    return Err(anyhow!("service disconnected"));
                }
            }

            let bytes = self.socket_buffer.split_to(self.current_write);
            self.sender.write(&bytes).map_err(|e| {
                self.terminate(TerminationReason::InternalError);
                e
            })?;
            self.current_write = 0;
        }

        match_transport!(pair {
            (tx, rx) => ProxyWithSecondary::new(self, tx, rx).run().await
        })
    }
}

/// A proxy for a session with both a primary and secondary connection
struct ProxyWithSecondary<
    PS: TransportSender,
    PR: TransportReceiver,
    SS: TransportSender,
    SR: TransportReceiver,
    P: ExecutorProviderInterface,
> {
    inner: Proxy<PS, PR, P>,
    secondary_sender: SS,
    secondary_receiver: SR,
}

impl<
    PS: TransportSender,
    PR: TransportReceiver,
    SS: TransportSender,
    SR: TransportReceiver,
    P: ExecutorProviderInterface,
> ProxyWithSecondary<PS, PR, SS, SR, P>
{
    fn new(inner: Proxy<PS, PR, P>, secondary_sender: SS, secondary_receiver: SR) -> Self {
        ProxyWithSecondary {
            inner,
            secondary_sender,
            secondary_receiver,
        }
    }

    fn terminate(&mut self, reason: TerminationReason) {
        self.secondary_sender
            .send(ResponseFrame::Termination { reason })
    }

    /// Main loop, handling incoming frames and outgoing bytes until the shutdown
    /// signal is received or an error occurs.
    async fn run(mut self) -> Result<()> {
        loop {
            tokio::select! {
                // Handle incoming payloads from the primary.
                // Primary connections should not be able to send service payloads anymore.
                res = self.inner.receiver.recv() => match res {
                    Some(req) => self.handle_primary_request(req).await?,
                    None => break Ok(()),
                },
                // Handle incoming payloads from the secondary.
                // Secondary connections should only be able to send service payloads.
                res = self.secondary_receiver.recv() => match res {
                    Some(req) => self.handle_secondary_request(req).await?,
                    None => break Ok(()),
                },
                // Handle outgoing socket bytes from the service to the secondary
                res = self.inner.socket.read_buf(&mut self.inner.socket_buffer) => match res {
                    Ok(0) => break Ok(()),
                    Ok(_) => {
                        handle_socket_bytes(
                            &mut self.inner.socket_buffer,
                            &mut self.inner.current_write,
                            &mut self.secondary_sender
                        )?
                    },
                    Err(_) => break Ok(()),
                },
                // Shutdown signal from the node
                _ = self.inner.context.shutdown.wait_for_shutdown() => break Ok(()),
            }
        }
    }

    /// Handle incoming request frame from the primary connection
    async fn handle_primary_request(&mut self, req: RequestFrame) -> Result<()> {
        match req {
            RequestFrame::ExtendAccessToken { ttl } => {
                self.inner
                    .context
                    .handle_extend_access_token_request(&self.inner.token, ttl)
                    .await
                    .map_err(|e| {
                        self.terminate(TerminationReason::InvalidToken);
                        e
                    })?;
            },
            RequestFrame::DeliveryAcknowledgment {} => todo!("verify and submit client DACK"),
            RequestFrame::AccessToken { .. } | RequestFrame::ServicePayload { .. } => {
                // should this be considered client misbehavior?
            },
            _ => unimplemented!(),
        }

        Ok(())
    }

    /// Handle incoming request frame from the secondary connection
    async fn handle_secondary_request(&mut self, req: RequestFrame) -> Result<()> {
        match req {
            RequestFrame::ServicePayload { bytes } => {
                self.inner
                    .socket
                    .write_u32(bytes.len() as u32)
                    .await
                    .map_err(|e| {
                        self.terminate(TerminationReason::InternalError);
                        e
                    })?;
                self.inner.socket.write_all(&bytes).await.map_err(|e| {
                    self.terminate(TerminationReason::InternalError);
                    e
                })?;
            },
            RequestFrame::AccessToken { .. }
            | RequestFrame::ExtendAccessToken { .. }
            | RequestFrame::DeliveryAcknowledgment { .. } => {
                // should this be considered client misbehavior?
            },
            _ => unimplemented!(),
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use anyhow::Result;
    use fleek_crypto::{ClientPublicKey, ClientSignature};
    use futures::{SinkExt, StreamExt};
    use lightning_interfaces::types::ServiceId;
    use lightning_interfaces::ExecutorProviderInterface;
    use lightning_schema::handshake::{
        HandshakeRequestFrame,
        RequestFrame,
        ResponseFrame,
        TerminationReason,
    };
    use tokio::net::UnixStream;
    use tokio::time::timeout;
    use tokio_util::codec::Framed;

    use crate::handshake::Context;
    use crate::shutdown::ShutdownNotifier;
    use crate::transports::mock::{dial_mock, MockTransport, MockTransportConfig};
    use crate::transports::Transport;

    const ECHO_SERVICE: u32 = 1001;
    const TEST_PAYLOAD: &[u8] = &[69; 420];

    #[derive(Clone)]
    struct MockServiceProvider;

    impl MockServiceProvider {
        fn echo_service(stream: UnixStream) {
            tokio::spawn(async move {
                let mut framed =
                    Framed::new(stream, tokio_util::codec::LengthDelimitedCodec::new());

                while let Some(Ok(bytes)) = framed.next().await {
                    if framed.send(bytes.into()).await.is_err() {
                        return;
                    }
                }
            });
        }
    }

    impl ExecutorProviderInterface for MockServiceProvider {
        async fn connect(&self, service_id: ServiceId) -> Option<UnixStream> {
            match service_id {
                ECHO_SERVICE => {
                    let (left, right) = UnixStream::pair().ok()?;
                    Self::echo_service(left);
                    Some(right)
                },
                _ => None,
            }
        }
    }

    async fn start_mock_node(id: u16) -> Result<ShutdownNotifier> {
        let shutdown = ShutdownNotifier::default();
        let context = Context::new(MockServiceProvider, shutdown.waiter());
        let (transport, _) =
            MockTransport::bind(shutdown.waiter(), MockTransportConfig { port: id }).await?;
        transport.spawn_listener_task(context);

        Ok(shutdown)
    }

    #[tokio::test]
    async fn primary_connection() -> Result<()> {
        // start and connect to the mock node
        let shutdown = start_mock_node(0).await?;
        let (tx, rx) = dial_mock(0).await.expect("failed to dial");

        // send handshake req
        tx.send(
            HandshakeRequestFrame::Handshake {
                retry: None,
                service: ECHO_SERVICE,
                pk: ClientPublicKey([0; 96]),
                pop: ClientSignature([0; 48]),
            }
            .encode(),
        )
        .await?;

        // interact with the service over the secondary connection
        for _ in 0..10 {
            tx.send(
                RequestFrame::ServicePayload {
                    bytes: TEST_PAYLOAD.into(),
                }
                .encode(),
            )
            .await?;

            match ResponseFrame::decode(&rx.recv().await?)? {
                ResponseFrame::ServicePayload { bytes } => assert_eq!(&bytes, TEST_PAYLOAD),
                f => panic!("expected payload, got {f:?}"),
            }
        }

        shutdown.shutdown();
        Ok(())
    }

    #[tokio::test]
    async fn join_secondary_connection() -> Result<()> {
        // start and connect to the mock node
        let shutdown = start_mock_node(1).await?;
        let (primary_tx, primary_rx) = dial_mock(1)
            .await
            .expect("failed to dial primary connection");

        // send handshake request
        primary_tx
            .send(
                HandshakeRequestFrame::Handshake {
                    retry: None,
                    service: ECHO_SERVICE,
                    pk: ClientPublicKey([0; 96]),
                    pop: ClientSignature([0; 48]),
                }
                .encode(),
            )
            .await?;

        // request and get access token
        primary_tx
            .send(RequestFrame::AccessToken { ttl: 1 }.encode())
            .await?;
        let access_token = match ResponseFrame::decode(&primary_rx.recv().await?)? {
            ResponseFrame::AccessToken { access_token, .. } => *access_token,
            f => panic!("expected access token, got {f:?}"),
        };

        // open secondary connection
        let (secondary_tx, secondary_rx) = dial_mock(1)
            .await
            .expect("failed to dial secondary connection");

        // send join request
        secondary_tx
            .send(HandshakeRequestFrame::JoinRequest { access_token }.encode())
            .await?;

        // interact with the service over the secondary connection
        for _ in 0..10 {
            secondary_tx
                .send(
                    RequestFrame::ServicePayload {
                        bytes: TEST_PAYLOAD.into(),
                    }
                    .encode(),
                )
                .await?;

            match ResponseFrame::decode(&secondary_rx.recv().await?)? {
                ResponseFrame::ServicePayload { bytes } => assert_eq!(&bytes, TEST_PAYLOAD),
                f => panic!("expected payload, got {f:?}"),
            }
        }

        shutdown.shutdown();
        Ok(())
    }

    #[tokio::test]
    async fn reject_expired_token() -> Result<()> {
        // start and connect to the mock node
        let shutdown = start_mock_node(2).await?;
        let (primary_tx, primary_rx) = dial_mock(2)
            .await
            .expect("failed to dial primary connection");

        // send handshake request
        primary_tx
            .send(
                HandshakeRequestFrame::Handshake {
                    retry: None,
                    service: ECHO_SERVICE,
                    pk: ClientPublicKey([0; 96]),
                    pop: ClientSignature([0; 48]),
                }
                .encode(),
            )
            .await?;

        // request and get access token
        primary_tx
            .send(RequestFrame::AccessToken { ttl: 1 }.encode())
            .await?;
        let access_token = match ResponseFrame::decode(&primary_rx.recv().await?)? {
            ResponseFrame::AccessToken { access_token, .. } => *access_token,
            f => panic!("expected access token, got {f:?}"),
        };

        // wait for the token to expire
        tokio::time::sleep(Duration::from_millis(1001)).await;

        // open secondary connection
        let (secondary_tx, secondary_rx) = dial_mock(2)
            .await
            .expect("failed to dial secondary connection");

        // send join request
        secondary_tx
            .send(HandshakeRequestFrame::JoinRequest { access_token }.encode())
            .await?;

        // connection should be immediately terminated
        let bytes = timeout(Duration::from_secs(1), secondary_rx.recv())
            .await
            .expect("termination frame should be sent within 1 second")?;
        assert_eq!(
            ResponseFrame::decode(&bytes)?,
            ResponseFrame::Termination {
                reason: TerminationReason::InvalidToken
            }
        );

        shutdown.shutdown();
        Ok(())
    }

    #[tokio::test]
    async fn extend_token() -> Result<()> {
        // start and connect to the mock node
        let shutdown = start_mock_node(3).await?;
        let (primary_tx, primary_rx) = dial_mock(3)
            .await
            .expect("failed to dial primary connection");

        // send handshake request
        primary_tx
            .send(
                HandshakeRequestFrame::Handshake {
                    retry: None,
                    service: ECHO_SERVICE,
                    pk: ClientPublicKey([0; 96]),
                    pop: ClientSignature([0; 48]),
                }
                .encode(),
            )
            .await?;

        // request and get access token
        primary_tx
            .send(RequestFrame::AccessToken { ttl: 1 }.encode())
            .await?;
        let access_token = match ResponseFrame::decode(&primary_rx.recv().await?)? {
            ResponseFrame::AccessToken { access_token, .. } => *access_token,
            f => panic!("expected access token, got {f:?}"),
        };

        // wait for the token to expire
        tokio::time::sleep(Duration::from_millis(1001)).await;

        // extend the token
        primary_tx
            .send(RequestFrame::ExtendAccessToken { ttl: 1 }.encode())
            .await?;

        // open secondary connection
        let (secondary_tx, secondary_rx) = dial_mock(3)
            .await
            .expect("failed to dial secondary connection");

        // send join request
        secondary_tx
            .send(HandshakeRequestFrame::JoinRequest { access_token }.encode())
            .await?;

        // interact with the service over the secondary connection
        for _ in 0..10 {
            secondary_tx
                .send(
                    RequestFrame::ServicePayload {
                        bytes: TEST_PAYLOAD.into(),
                    }
                    .encode(),
                )
                .await?;

            match ResponseFrame::decode(&secondary_rx.recv().await?)? {
                ResponseFrame::ServicePayload { bytes } => assert_eq!(&bytes, TEST_PAYLOAD),
                f => panic!("expected payload, got {f:?}"),
            }
        }

        shutdown.shutdown();
        Ok(())
    }
}
