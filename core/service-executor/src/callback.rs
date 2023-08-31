use fn_sdk::internal::{IpcRequest, OnEventResponseArgs};
use lightning_interfaces::ConnectionWork;

use crate::deque::CommandSender;

/// Create a callback for us to pass to the setup function of the service.
pub fn make_callback(
    scheduler: CommandSender,
    cb: fn(OnEventResponseArgs),
) -> Box<dyn Fn(IpcRequest) + Send + Sync> {
    Box::new(move |request: IpcRequest| {
        let scheduler = scheduler.clone();
        tokio::spawn(handle_request(cb, scheduler, request));
    })
}

#[inline(always)]
async fn handle_request(
    _cb: fn(OnEventResponseArgs),
    scheduler: CommandSender,
    IpcRequest {
        request_ctx,
        request,
    }: IpcRequest,
) {
    match request {
        fn_sdk::internal::Request::ConnectionClose { connection_id } => {
            scheduler.put(ConnectionWork::Close { connection_id }).await;
            assert!(request_ctx.is_none());
        },
        fn_sdk::internal::Request::ConnectionSendData {
            connection_id,
            buffer,
        } => {
            scheduler
                .put(ConnectionWork::Send {
                    connection_id,
                    payload: buffer,
                })
                .await;
            assert!(request_ctx.is_none());
        },
        fn_sdk::internal::Request::QueryClientBalance { pk: _ } => todo!(),
        fn_sdk::internal::Request::FetchBlake3 { hash: _ } => todo!(),
        _ => todo!(),
    };
}
