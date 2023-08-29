use fleek_crypto::ClientPublicKey;

use crate::futures::RequestFuture;
use crate::internal::{IpcRequest, OnEventResponseArgs, OnStartArgs, Request};

static mut SENDER: Option<Box<dyn Fn(IpcRequest) + Send + Sync>> = None;

/// This method should always be called during the initialization of the SDK context.
pub fn setup(args: OnStartArgs) {
    unsafe {
        assert!(SENDER.is_none(), "setup already done");
        SENDER = Some(args.request_sender);
    }
}

/// Handle a response from core.
pub fn on_event_response(args: OnEventResponseArgs) {
    crate::futures::future_callback(args.request_ctx, args.response);
}

fn send_and_await_response(request: Request) -> RequestFuture {
    let (request_ctx, future) = crate::futures::create_future();
    unsafe {
        let sender = SENDER.as_ref().expect("setup not completed");
        (*sender)(IpcRequest {
            request_ctx,
            request,
        });
    }
    future
}

pub async fn query_client_balance(pk: ClientPublicKey) -> u128 {
    let req = Request::QueryClientBalance { pk };
    let res = send_and_await_response(req).await;
    match res {
        crate::internal::Response::QueryClientBalance { balance } => balance,
        _ => unreachable!(),
    }
}

#[test]
fn x() {
    setup(OnStartArgs {
        request_sender: Box::new(|req| {
            println!("got a request {:?}", req);
            on_event_response(OnEventResponseArgs {
                request_ctx: req.request_ctx,
                response: crate::internal::Response::QueryClientBalance { balance: 135 },
            });
        }),
        block_store_path: "/Users/parsa/tmp".try_into().unwrap(),
    });
    futures::executor::block_on(async {
        let balance = query_client_balance(ClientPublicKey([0; 96])).await;
        println!("balance = {balance}");
    });
}
