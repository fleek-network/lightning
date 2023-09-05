use std::path::PathBuf;

use fleek_crypto::ClientPublicKey;
use futures::Future;
use tokio::task::JoinHandle;

use crate::futures::RequestFuture;
use crate::internal::{IpcRequest, OnEventResponseArgs, OnStartArgs, Request};

static mut SENDER: Option<Box<dyn Fn(IpcRequest) + Send + Sync>> = None;
static mut BLOCKSTORE: Option<PathBuf> = None;

/// This method should always be called during the initialization of the SDK context.
pub fn setup(args: OnStartArgs) {
    unsafe {
        assert!(SENDER.is_none(), "setup already done");
        SENDER = Some(args.request_sender);
        BLOCKSTORE = Some(args.block_store_path);
    }
}

pub(crate) fn blockstore_root() -> &'static PathBuf {
    unsafe { BLOCKSTORE.as_ref().expect("setup not completed") }
}

/// Handle a response from core.
pub fn on_event_response(args: OnEventResponseArgs) {
    crate::futures::future_callback(args.request_ctx, args.response);
}

fn send_no_response(request: Request) {
    unsafe {
        let sender = SENDER.as_ref().expect("setup not completed");
        (*sender)(IpcRequest {
            request_ctx: None,
            request,
        });
    }
}

fn send_and_await_response(request: Request) -> RequestFuture {
    let (request_ctx, future) = crate::futures::create_future();
    unsafe {
        let sender = SENDER.as_ref().expect("setup not completed");
        (*sender)(IpcRequest {
            request_ctx: Some(request_ctx),
            request,
        });
    }
    future
}

/// Spawn the future until completion. Top level event handlers should use this method
/// if they need to perform async operations.
pub fn spawn<T>(future: T) -> JoinHandle<T::Output>
where
    T: Future + Send + 'static,
    T::Output: Send + 'static,
{
    if let Ok(handle) = tokio::runtime::Handle::try_current() {
        handle.spawn(future)
    } else {
        // Right now we're running services by direct call from core. Since
        // in core we're already running within a tokio runtime, we wouldn't
        // need to handle this case for the time being.
        todo!()
    }
}

/// Close a connection using the connection id.
pub fn connection_close(connection_id: u64) {
    send_no_response(Request::ConnectionClose { connection_id });
}

/// Write the provided buffer to the connection.
pub fn connection_send(connection_id: u64, buffer: Vec<u8>) {
    send_no_response(Request::ConnectionSendData {
        connection_id,
        buffer,
    });
}

pub async fn query_client_balance(pk: ClientPublicKey) -> u128 {
    let req = Request::QueryClientBalance { pk };
    let res = send_and_await_response(req).await;
    match res {
        crate::internal::Response::QueryClientBalance { balance } => balance,
        _ => unreachable!(),
    }
}

#[tokio::test]
async fn x() {
    setup(OnStartArgs {
        request_sender: Box::new(|req| {
            println!("got a request {:?}", req);
            let Some(request_ctx) = req.request_ctx else {
                return;
            };
            on_event_response(OnEventResponseArgs {
                request_ctx,
                response: crate::internal::Response::QueryClientBalance { balance: 135 },
            });
        }),
        block_store_path: "/Users/parsa/tmp".try_into().unwrap(),
    });

    let balance = query_client_balance(ClientPublicKey([0; 96])).await;
    println!("balance = {balance}");
}
