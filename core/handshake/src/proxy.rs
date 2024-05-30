use std::collections::VecDeque;
use std::time::Duration;

use arrayref::array_ref;
use async_channel::Receiver;
use bytes::BytesMut;
use lightning_interfaces::schema::handshake::{ResponseFrame, TerminationReason};
use lightning_interfaces::{spawn, ExecutorProviderInterface};
use lightning_metrics::increment_counter;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;

use crate::handshake::Context;
use crate::schema::RequestFrame;
use crate::transports::{match_transport, TransportPair, TransportReceiver, TransportSender};

pub struct Proxy<P: ExecutorProviderInterface> {
    context: Context<P>,
    /// The id for this connection.
    connection_id: u64,
    /// The id for the service this connection is connected to.
    service_id: u32,
    /// The unix socket connection to the service made specifically for this ongoing connection.
    socket: UnixStream,
    /// The buffer using which we read bytes from the unix socket.
    buffer: BytesMut,
    //// A socket used to make us aware of new connection requests or refresh by the client.
    connection_rx: Receiver<(IsPrimary, TransportPair)>,
    /// The size of the current service payload we're writing from the service socket to the client
    /// transport.
    current_write: usize,
    /// If this is set to true it indicates that the current payload (`current_write` many bytes)
    /// should be treated as garbage.
    discard_bytes: bool,
    /// If this is set to true it indicates that the current active writer is actually the primary
    /// connection in the case of the co-existence of primary and secondary. This happens when the
    /// secondary connection happens while we are in the middle of sending something to the
    /// primary.
    is_primary_the_current_sender: IsPrimary,
    /// Only [`ResponseFrame::AccessToken`]s that are meant to be sent to the primary. The reason
    /// we have to queue these here is that at times we may be in the middle of sending a service
    /// payload through the transport. And randomly inserting in some other frame in the middle of
    /// an active length delimited message before reaching the promised length breaks many things.
    queued_primary_response: VecDeque<ResponseFrame>,
    timeout: Duration,
}

pub type IsPrimary = bool;

pub enum State {
    NoConnection,
    OnlyPrimaryConnection(TransportPair),
    OnlySecondaryConnection(TransportPair),
    PrimaryAndSecondary(TransportPair, TransportPair),
    Terminated,
}

enum HandleRequestResult {
    Ok,
    DropTransport,
    TerminateConnection,
}

impl<P: ExecutorProviderInterface> Proxy<P> {
    #[allow(clippy::too_many_arguments)]
    #[inline(always)]
    pub fn new(
        connection_id: u64,
        service_id: u32,
        socket: UnixStream,
        connection_rx: Receiver<(IsPrimary, TransportPair)>,
        context: Context<P>,
        timeout: Duration,
    ) -> Self {
        Self {
            context,
            connection_id,
            service_id,
            socket,
            buffer: Default::default(),
            connection_rx,
            current_write: 0,
            discard_bytes: false,
            is_primary_the_current_sender: false,
            queued_primary_response: VecDeque::new(),
            timeout,
        }
    }

    #[inline(always)]
    pub fn spawn(self, start: Option<State>) {
        spawn!(self.run(start), "HANDSHAKE: proxy spawn");
    }

    #[inline(always)]
    pub async fn run(mut self, start: Option<State>) {
        // Run the main loop of the state each iteration of this loop switches between different
        // cases and implementations depending on the connections and their existence. And does
        // not actually handle any of the events. It is simply the bigger state machine deciding
        // with smaller state machine to use lol.
        //
        // But honestly this could have been implemented better and easier if async recursion was
        // supported better and the current implementation is simply a workaround around not being
        // to properly using async recursion.
        let mut state = start.unwrap_or(State::NoConnection);
        loop {
            state = match state {
                State::NoConnection => self.run_with_no_connection().await,
                State::OnlyPrimaryConnection(pair) => {
                    match_transport!(pair {
                        (tx, rx) => self.run_with_primary(tx, rx).await
                    })
                },
                State::OnlySecondaryConnection(pair) => {
                    match_transport!(pair {
                        (tx, rx) => self.run_with_secondary(tx, rx).await
                    })
                },
                State::PrimaryAndSecondary(p, s) => {
                    // Warning: generic explosion ahead! xD
                    match_transport!(p {
                        (ps, pr) => match_transport!(s {
                            (ss, sr) => self.run_with_primary_and_secondary(ps, pr, ss, sr).await
                        })
                    })
                },
                State::Terminated => {
                    break;
                },
            };
        }
    }

    #[inline]
    async fn run_with_no_connection(&mut self) -> State {
        match tokio::time::timeout(self.timeout, self.connection_rx.recv()).await {
            Ok(Ok((is_primary, pair))) => {
                if is_primary {
                    State::OnlyPrimaryConnection(pair)
                } else {
                    State::OnlySecondaryConnection(pair)
                }
            },
            Ok(Err(_)) | Err(_) => State::Terminated,
        }
    }

    #[inline]
    async fn run_with_primary<S: TransportSender, R: TransportReceiver>(
        &mut self,
        sender: S,
        receiver: R,
    ) -> State
    where
        TransportPair: From<(S, R)>,
    {
        self.run_with_one_connection(true, sender, receiver).await
    }

    #[inline]
    async fn run_with_secondary<S: TransportSender, R: TransportReceiver>(
        &mut self,
        sender: S,
        receiver: R,
    ) -> State
    where
        TransportPair: From<(S, R)>,
    {
        self.run_with_one_connection(false, sender, receiver).await
    }

    #[inline(always)]
    async fn run_with_one_connection<S: TransportSender, R: TransportReceiver>(
        &mut self,
        is_primary: IsPrimary,
        mut sender: S,
        mut receiver: R,
    ) -> State
    where
        TransportPair: From<(S, R)>,
    {
        let reason = 'outer: loop {
            self.grow_buffer();

            tokio::select! {
                res = receiver.recv() => {
                    match async_map(res, |r| self.handle_incoming(is_primary, r)).await {
                        Some(HandleRequestResult::Ok) if is_primary => {
                            self.maybe_flush_primary_queue(true, &mut sender).await;
                        },
                        Some(HandleRequestResult::Ok) => {},
                        Some(HandleRequestResult::TerminateConnection) => {
                            break 'outer TerminationReason::InternalError;
                        },
                        Some(HandleRequestResult::DropTransport) | None => {
                            // We're possibly switching connection. If there are any pending bytes from
                            // a current service payload we need to discard them before moving on to
                            // send the next payload to the new connection.
                            self.discard_bytes = true;
                            self.queued_primary_response.clear();
                            return State::NoConnection;
                        }
                    }
                },
                res = self.connection_rx.recv() => match res {
                    Ok((is_new_primary, pair)) => return match (is_primary, is_new_primary) {
                        // Drop the previous connection if a new attempt is being made to reuse
                        // the same connection mode. The client must have lost its connection.
                        // We terminate the current (old) connection with a `ConnectionInUse` error.
                        (true, true) => {
                            sender.terminate(TerminationReason::ConnectionInUse).await;
                            self.discard_bytes = true;
                            self.queued_primary_response.clear();
                            State::OnlyPrimaryConnection(pair)
                        },
                        (false, false) => {
                            sender.terminate(TerminationReason::ConnectionInUse).await;
                            self.discard_bytes = true;
                            State::OnlySecondaryConnection(pair)
                        }
                        (true, false) => {
                            // the current connection is a primary, a secondary always takes
                            // priority to be the active sender, but if we have a pending
                            // payload going out we should still keep this primary as the active
                            // writer.
                            self.is_primary_the_current_sender = true;
                            State::PrimaryAndSecondary((sender, receiver).into(), pair)
                        },
                        (false, true) => {
                            self.is_primary_the_current_sender = false;
                            State::PrimaryAndSecondary(pair,(sender, receiver).into())
                        },
                    },
                    Err(_) => {
                        break 'outer TerminationReason::InternalError;
                    }
                },
                res = self.socket.read_buf(&mut self.buffer) => match res {
                    Ok(0) => {
                        debug_assert_ne!(self.buffer.capacity(), 0);
                        break if self.current_write == 0 {
                            TerminationReason::ServiceTerminated
                        } else {
                            TerminationReason::InternalError
                        }
                    },
                    Err(_) => { break TerminationReason::InternalError },
                    _ => {
                        'inner: while !self.buffer.is_empty() {
                            // current write is set to zero. we're looking for the next header.
                            if self.current_write == 0 {
                                if self.buffer.len() < 4 {
                                    // we have fewer bytes to do anything with.
                                    continue 'outer;
                                }

                                // When we are here it means we're reading a new message so any
                                // assumption about the previous payload has to be reset.
                                self.is_primary_the_current_sender = false;
                                self.discard_bytes = false;

                                // This might be a window to flush some pending responses to the
                                // primary.
                                if is_primary {
                                    self.maybe_flush_primary_queue(true, &mut sender).await;
                                }

                                let bytes = self.buffer.split_to(4);
                                let len = u32::from_be_bytes(*array_ref![bytes, 0, 4]) as usize;
                                sender.start_write(len).await;
                                self.current_write = len;
                                continue 'inner; // to handle `len` == 0.
                            }

                            let take = self.current_write.min(self.buffer.len());
                            let bytes = self.buffer.split_to(take);
                            self.current_write -= take;

                            if self.discard_bytes {
                                // Just ignore these bytes and move on.
                                continue 'inner;
                            }

                            if sender.write(bytes.freeze()).await.is_err() {
                                self.discard_bytes = true;
                                self.queued_primary_response.clear();
                                return State::NoConnection;
                            }
                        }
                    }
                }
            }
        };

        sender.terminate(reason).await;
        State::Terminated
    }

    #[inline]
    pub async fn run_with_primary_and_secondary<
        PS: TransportSender,
        PR: TransportReceiver,
        SS: TransportSender,
        SR: TransportReceiver,
    >(
        &mut self,
        mut p_sender: PS,
        mut p_receiver: PR,
        mut s_sender: SS,
        mut s_receiver: SR,
    ) -> State
    where
        TransportPair: From<(PS, PR)> + From<(SS, SR)>,
    {
        let reason = 'outer: loop {
            self.grow_buffer();

            tokio::select! {
                res = p_receiver.recv() => {
                    match async_map(res, |r| self.handle_incoming(true, r)).await {
                        Some(HandleRequestResult::Ok) => {
                            self.maybe_flush_primary_queue(false, &mut p_sender).await;
                        },
                        Some(HandleRequestResult::TerminateConnection) => {
                            break 'outer TerminationReason::InternalError;
                        },
                        Some(HandleRequestResult::DropTransport) | None => {
                            // We lost connection with primary. So if we're currently writing to it
                            // discard the current payload going its way.
                            if self.is_primary_the_current_sender {
                                self.discard_bytes = true;
                            }
                            self.queued_primary_response.clear();
                            return State::OnlySecondaryConnection((s_sender, s_receiver).into());
                        }
                    }
                },
                res = s_receiver.recv() => {
                    match async_map(res, |r| self.handle_incoming(false, r)).await {
                        Some(HandleRequestResult::Ok) => {},
                        Some(HandleRequestResult::TerminateConnection) => {
                            break 'outer TerminationReason::InternalError;
                        },
                        Some(HandleRequestResult::DropTransport) | None => {
                         if !self.is_primary_the_current_sender {
                                self.discard_bytes = true;
                            }
                            return State::OnlyPrimaryConnection((p_sender, p_receiver).into());
                        }
                    }
                },
                res = self.connection_rx.recv() => match res {
                    Ok((is_primary, pair)) => {
                        return match (is_primary, self.is_primary_the_current_sender) {
                            (true, true) => {
                                self.discard_bytes = true;
                                self.queued_primary_response.clear();
                                State::PrimaryAndSecondary(pair, (s_sender, s_receiver).into())
                            },
                            (true, false) => {
                                self.queued_primary_response.clear();
                                State::PrimaryAndSecondary(pair, (s_sender, s_receiver).into())
                            },
                            (false, true) => {
                                State::PrimaryAndSecondary((p_sender, p_receiver).into(), pair)
                            },
                            (false, false) => {
                                self.discard_bytes = true;
                                State::PrimaryAndSecondary((p_sender, p_receiver).into(), pair)
                            }
                        }
                    },
                    Err(_) => {
                        break TerminationReason::InternalError;
                    }
                },
                res = self.socket.read_buf(&mut self.buffer) => match res {
                    Ok(0) => {
                        debug_assert_ne!(self.buffer.capacity(), 0);
                        break if self.current_write == 0 {
                            TerminationReason::ServiceTerminated
                        } else {
                            TerminationReason::InternalError
                        }
                    },
                    Err(_) => { break TerminationReason::InternalError },
                    _ => {
                        'inner: while !self.buffer.is_empty() {
                            // current write is set to zero. we're looking for the next header.
                            if self.current_write == 0 {
                                if self.buffer.len() < 4 {
                                    // we have fewer bytes to do anything with.
                                    continue 'outer;
                                }

                                // When we are here, it means we're reading a new message so any
                                // assumption about the previous payload has to be reset.
                                self.is_primary_the_current_sender = false;
                                self.discard_bytes = false;

                                // This might be a window to flush some pending responses to the
                                // primary.
                                self.maybe_flush_primary_queue(false, &mut p_sender).await;

                                let bytes = self.buffer.split_to(4);
                                let len = u32::from_be_bytes(*array_ref![bytes, 0, 4]) as usize;
                                s_sender.start_write(len).await;
                                self.current_write = len;
                                continue 'inner; // to handle `len` == 0.
                            }

                            let take = self.current_write.min(self.buffer.len());
                            let bytes = self.buffer.split_to(take);
                            self.current_write -= take;

                            if self.discard_bytes {
                                // Just ignore these bytes and move on.
                                continue 'inner;
                            }

                            return if self.is_primary_the_current_sender {
                                if p_sender.write(bytes.freeze()).await.is_ok() {
                                    continue 'inner;
                                }
                                self.discard_bytes = true;
                                self.queued_primary_response.clear();
                                State::OnlySecondaryConnection((s_sender, s_receiver).into())
                            } else {
                                if s_sender.write(bytes.freeze()).await.is_ok() {
                                    continue 'inner;
                                }
                                self.discard_bytes = true;
                                State::OnlyPrimaryConnection((p_sender, p_receiver).into())
                            }
                        }
                    }
                }
            }
        };

        s_sender.terminate(reason).await;
        p_sender.terminate(reason).await;
        State::Terminated
    }

    async fn handle_incoming(
        &mut self,
        is_primary: IsPrimary,
        request: RequestFrame,
    ) -> HandleRequestResult {
        match request {
            RequestFrame::ServicePayload { bytes } => {
                let service_id = self.service_id.to_string();
                increment_counter!(
                    "handshake_service_payloads",
                    Some("Counter for the number of service payloads received"),
                    "service_id" => service_id.as_str()
                );
                if self.socket.write_u32(bytes.len() as u32).await.is_err() {
                    return HandleRequestResult::TerminateConnection;
                }
                if self.socket.write_all(&bytes).await.is_err() {
                    return HandleRequestResult::TerminateConnection;
                }
                HandleRequestResult::Ok
            },
            RequestFrame::AccessToken { .. } if !is_primary => HandleRequestResult::DropTransport,
            RequestFrame::ExtendAccessToken { .. } if !is_primary => {
                HandleRequestResult::DropTransport
            },
            RequestFrame::AccessToken { ttl } => {
                let (access_token, ttl) = self.context.extend_access_token(self.connection_id, ttl);
                self.queued_primary_response
                    .push_front(ResponseFrame::AccessToken {
                        ttl,
                        access_token: access_token.into(),
                    });
                HandleRequestResult::Ok
            },
            RequestFrame::ExtendAccessToken { ttl } => {
                self.context.extend_access_token(self.connection_id, ttl);
                HandleRequestResult::Ok
            },
            RequestFrame::DeliveryAcknowledgment { .. } => {
                // todo: not supported/expected at the moment.
                HandleRequestResult::DropTransport
            },
            _ => unreachable!(),
        }
    }

    /// Makes sure the buffer has a proper allocated capacity based on the expected number of bytes.
    #[inline(always)]
    fn grow_buffer(&mut self) {
        let current_buf_len = self.buffer.len();
        let expected_future_len = (current_buf_len + self.current_write).next_power_of_two();
        let additional = expected_future_len - current_buf_len;
        if expected_future_len < 128 {
            self.buffer.reserve(128 - current_buf_len);
        } else if expected_future_len > 4096 {
            self.buffer
                .reserve(4096usize.saturating_sub(current_buf_len));
        } else if additional > 0 {
            self.buffer.reserve(additional);
        }
    }

    #[inline(always)]
    async fn maybe_flush_primary_queue<S: TransportSender>(
        &mut self,
        only_primary: bool,
        sender: &mut S,
    ) {
        if self.queued_primary_response.is_empty() {
            return;
        }

        let is_primary_current_writer = only_primary || self.is_primary_the_current_sender;
        if !is_primary_current_writer || self.current_write == 0 || self.discard_bytes {
            while let Some(res) = self.queued_primary_response.pop_back() {
                sender.send(res).await;
            }
        }
    }
}

impl<P: ExecutorProviderInterface> Drop for Proxy<P> {
    fn drop(&mut self) {
        self.context.cleanup_connection(self.connection_id);
    }
}

#[inline(always)]
async fn async_map<T, F, O, R>(option: Option<T>, f: F) -> Option<R>
where
    F: FnOnce(T) -> O,
    O: std::future::Future<Output = R>,
{
    if let Some(value) = option {
        Some(f(value).await)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use anyhow::Result;
    use fleek_crypto::{ClientPublicKey, ClientSignature};
    use fn_sdk::header::read_header;
    use futures::{SinkExt, StreamExt};
    use lightning_interfaces::prelude::*;
    use lightning_interfaces::schema::handshake::{
        HandshakeRequestFrame,
        RequestFrame,
        ResponseFrame,
        TerminationReason,
    };
    use lightning_interfaces::types::ServiceId;
    use lightning_interfaces::ShutdownController;
    use tokio::net::UnixStream;
    use tokio::time::timeout;
    use tokio_util::codec::Framed;

    use crate::handshake::Context;
    use crate::transports::mock::{dial_mock, MockTransport, MockTransportConfig};
    use crate::transports::Transport;

    const ECHO_SERVICE: u32 = 1001;
    const TEST_PAYLOAD: &[u8] = &[69; 420];

    #[derive(Clone)]
    struct MockServiceProvider;

    impl MockServiceProvider {
        fn echo_service(mut stream: UnixStream) {
            spawn!(
                async move {
                    read_header(&mut stream)
                        .await
                        .expect("Could not read hello frame.");

                    let mut framed =
                        Framed::new(stream, tokio_util::codec::LengthDelimitedCodec::new());

                    while let Some(Ok(bytes)) = framed.next().await {
                        if framed.send(bytes.into()).await.is_err() {
                            return;
                        }
                    }
                },
                "HANDSHAKE: echo service"
            );
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

    async fn start_mock_node<P: ExecutorProviderInterface>(id: u16) -> Result<ShutdownController> {
        let shutdown = ShutdownController::default();
        let context = Context::new(
            MockServiceProvider,
            shutdown.waiter(),
            Duration::from_secs(1),
        );
        let (transport, _) =
            MockTransport::bind::<P>(shutdown.waiter(), MockTransportConfig { port: id }).await?;
        transport.spawn_listener_task(context);

        Ok(shutdown)
    }

    #[tokio::test]
    async fn primary_connection() -> Result<()> {
        // start and connect to the mock node
        let mut shutdown = start_mock_node::<MockServiceProvider>(0).await?;
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

        shutdown.shutdown().await;
        Ok(())
    }

    #[tokio::test]
    async fn join_secondary_connection() -> Result<()> {
        // start and connect to the mock node
        let mut shutdown = start_mock_node::<MockServiceProvider>(1).await?;
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

        shutdown.shutdown().await;
        Ok(())
    }

    #[tokio::test]
    async fn reject_expired_token() -> Result<()> {
        // start and connect to the mock node
        let mut shutdown = start_mock_node::<MockServiceProvider>(2).await?;
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

        shutdown.shutdown().await;
        Ok(())
    }

    #[tokio::test]
    async fn extend_token() -> Result<()> {
        // start and connect to the mock node
        let mut shutdown = start_mock_node::<MockServiceProvider>(3).await?;
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

        shutdown.shutdown().await;
        Ok(())
    }
}
